//! Minimal Stratum v1 client (solo pools such as public-pool.io / ckpool).

use crate::state::Shared;
use crate::work::{Job, stratum_prevhash};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::Ordering::*;
use std::time::{Duration, Instant};

const AGENT: &str = concat!("NerdMinerEmu/", env!("CARGO_PKG_VERSION"));

pub fn spawn(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("stratum".into())
        .spawn(move || {
            loop {
                let generation = shared.config_gen.load(Acquire);
                let cfg = shared.config.read().unwrap().clone();
                if cfg.btc_address.trim().is_empty() {
                    shared.set_status("Configure seu endereço BTC");
                    shared.pool_connected.store(false, Relaxed);
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }
                if let Err(e) = session(&shared, generation) {
                    shared.log(format!("Pool: {e}"));
                    shared.set_status(format!("Erro: {e}"));
                }
                shared.pool_connected.store(false, Relaxed);
                *shared.job.write().unwrap() = None;
                shared.job_gen.fetch_add(1, Release);
                // Wait a little before reconnecting unless config changed.
                for _ in 0..50 {
                    if shared.config_gen.load(Acquire) != generation {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        })
        .expect("spawn stratum thread");
}

fn session(shared: &Arc<Shared>, generation: u64) -> Result<(), String> {
    let cfg = shared.config.read().unwrap().clone();
    let addr = format!("{}:{}", cfg.pool_host.trim(), cfg.pool_port);
    shared.set_status(format!("Conectando a {addr}…"));
    let sock = addr
        .to_socket_addrs()
        .map_err(|e| format!("DNS {addr}: {e}"))?
        .next()
        .ok_or("endereço inválido")?;
    let stream = TcpStream::connect_timeout(&sock, Duration::from_secs(10)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_millis(200))).ok();
    stream.set_nodelay(true).ok();
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);

    let mut send = |v: Value| -> Result<(), String> {
        let mut s = v.to_string();
        s.push('\n');
        writer.write_all(s.as_bytes()).map_err(|e| e.to_string())
    };

    let user = cfg.username();
    send(json!({"id": 1, "method": "mining.subscribe", "params": [AGENT]}))?;

    let mut extranonce1: Vec<u8> = vec![];
    let mut en2_size = 4usize;
    let mut pool_diff = 1.0f64;
    let mut next_id = 10u64;
    let mut pending: HashMap<u64, f64> = HashMap::new();
    let mut line = String::new();
    let mut last_rx = Instant::now();

    // drop stale submissions from a previous session
    while shared.submit_rx.try_recv().is_ok() {}

    loop {
        if shared.config_gen.load(Acquire) != generation {
            return Err("configuração alterada, reconectando".into());
        }

        // Forward found shares.
        while let Ok(sub) = shared.submit_rx.try_recv() {
            let cur = shared.job.read().unwrap().as_ref().map(|j| j.job_id.clone());
            if cur.as_deref() != Some(sub.job_id.as_str()) {
                continue; // stale
            }
            let id = next_id;
            next_id += 1;
            pending.insert(id, sub.diff);
            send(json!({"id": id, "method": "mining.submit", "params": [
                user, sub.job_id, hex::encode(&sub.extranonce2),
                format!("{:08x}", sub.ntime), format!("{:08x}", sub.nonce)
            ]}))?;
            shared.log(format!("Share enviado (dif {})", crate::fmt::diff(sub.diff)));
        }

        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return Err("conexão encerrada pela pool".into()),
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if last_rx.elapsed() > Duration::from_secs(300) {
                    return Err("sem resposta da pool".into());
                }
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
        last_rx = Instant::now();
        let msg: Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(method) = msg.get("method").and_then(Value::as_str) {
            let params = msg.get("params").cloned().unwrap_or(Value::Null);
            match method {
                "mining.set_difficulty" => {
                    if let Some(d) = params.get(0).and_then(Value::as_f64) {
                        pool_diff = d;
                        shared.pool_diff.set(d);
                        shared.log(format!("Dificuldade da pool: {}", crate::fmt::diff(d)));
                    }
                }
                "mining.notify" => {
                    if let Some(job) = parse_notify(&params, &extranonce1, en2_size, pool_diff) {
                        let clean = params.get(8).and_then(Value::as_bool).unwrap_or(true);
                        *shared.job.write().unwrap() = Some(Arc::new(job));
                        shared.templates.fetch_add(1, Relaxed);
                        if clean {
                            shared.extranonce2_counter.store(0, Relaxed);
                        }
                        shared.job_gen.fetch_add(1, Release);
                    }
                }
                "mining.set_extranonce" => {
                    if let Some(e1) = params.get(0).and_then(Value::as_str) {
                        extranonce1 = hex::decode(e1).unwrap_or_default();
                    }
                    if let Some(n) = params.get(1).and_then(Value::as_u64) {
                        en2_size = n as usize;
                    }
                }
                "client.reconnect" => return Err("pool pediu reconexão".into()),
                _ => {}
            }
            continue;
        }

        let id = msg.get("id").and_then(Value::as_u64).unwrap_or(0);
        let result = msg.get("result").cloned().unwrap_or(Value::Null);
        let error = msg.get("error").cloned().unwrap_or(Value::Null);
        match id {
            1 => {
                let e1 = result.get(1).and_then(Value::as_str).ok_or("subscribe inválido")?;
                extranonce1 = hex::decode(e1).map_err(|e| e.to_string())?;
                en2_size = result.get(2).and_then(Value::as_u64).unwrap_or(4) as usize;
                if cfg.suggest_diff > 0.0 {
                    send(json!({"id": 3, "method": "mining.suggest_difficulty", "params": [cfg.suggest_diff]}))?;
                }
                send(json!({"id": 2, "method": "mining.authorize", "params": [user, cfg.pool_password]}))?;
            }
            2 => {
                if result.as_bool() == Some(true) {
                    shared.pool_connected.store(true, Relaxed);
                    shared.set_status(format!("Conectado: {}", cfg.pool_host));
                    shared.log(format!("Autorizado como {user}"));
                } else {
                    return Err(format!("autorização recusada: {error}"));
                }
            }
            3 => {}
            _ => {
                if let Some(d) = pending.remove(&id) {
                    if result.as_bool() == Some(true) {
                        shared.accepted.fetch_add(1, Relaxed);
                        shared.log(format!("Share ACEITO (dif {})", crate::fmt::diff(d)));
                    } else {
                        shared.rejected.fetch_add(1, Relaxed);
                        shared.log(format!("Share rejeitado: {error}"));
                    }
                }
            }
        }
    }
}

fn parse_notify(p: &Value, en1: &[u8], en2_size: usize, pool_diff: f64) -> Option<Job> {
    let s = |i: usize| p.get(i).and_then(Value::as_str);
    let hex_u32 = |i: usize| s(i).and_then(|x| u32::from_str_radix(x, 16).ok());
    let branches = p
        .get(4)?
        .as_array()?
        .iter()
        .filter_map(|b| {
            let v = hex::decode(b.as_str()?).ok()?;
            v.try_into().ok()
        })
        .collect();
    Some(Job {
        job_id: s(0)?.to_string(),
        prevhash: stratum_prevhash(s(1)?)?,
        coinb1: hex::decode(s(2)?).ok()?,
        coinb2: hex::decode(s(3)?).ok()?,
        extranonce1: en1.to_vec(),
        extranonce2_size: en2_size,
        merkle_branch: branches,
        version: hex_u32(5)?,
        nbits: hex_u32(6)?,
        ntime: hex_u32(7)?,
        pool_diff,
    })
}
