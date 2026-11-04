//! GPU miner using a wgpu compute shader (Metal / Vulkan / DX12).

use crate::i18n::{fill, t};
use crate::state::{GpuStatus, Shared};
use crate::work::{WorkUnit, top32_threshold, verify};
use std::sync::Arc;
use std::sync::atomic::Ordering::*;
use std::time::{Duration, Instant};

const WG: u32 = 256;
const MAX_WG: u32 = 65535;
/// Target duration of one dispatch, keeps the desktop responsive.
const TARGET_DISPATCH: f64 = 0.03;
const OUT_WORDS: usize = 65;

pub struct GpuMiner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    params: wgpu::Buffer,
    out: wgpu::Buffer,
    readback: wgpu::Buffer,
    bind: wgpu::BindGroup,
    pub name: String,
}

impl GpuMiner {
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| fill(t().gpu_none, &[&e]))?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("nerdminer"),
            ..Default::default()
        }))
        .map_err(|e| format!("request_device: {e}"))?;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sha256d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sha256d.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("sha256d"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params"),
            size: 13 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("out"),
            size: (OUT_WORDS * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (OUT_WORDS * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: out.as_entire_binding() },
            ],
        });
        Ok(GpuMiner {
            device,
            queue,
            pipeline,
            params,
            out,
            readback,
            bind,
            name: format!("{} ({:?})", info.name, info.backend),
        })
    }

    /// Hashes `count` nonces (multiple of 256) starting at `base`; returns candidate nonces.
    pub fn run(&self, w: &WorkUnit, base: u32, count: u32) -> Vec<u32> {
        let be = |i: usize| u32::from_be_bytes(w.header[i..i + 4].try_into().unwrap());
        let mut p = [0u32; 13];
        p[..8].copy_from_slice(&w.midstate);
        p[8] = be(64);
        p[9] = be(68);
        p[10] = be(72);
        p[11] = base;
        p[12] = top32_threshold(w.pool_diff);
        self.queue.write_buffer(&self.params, 0, bytemuck::cast_slice(&p));
        self.queue.write_buffer(&self.out, 0, &[0u8; 4]);

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.dispatch_workgroups((count / WG).clamp(1, MAX_WG), 1, 1);
        }
        enc.copy_buffer_to_buffer(&self.out, 0, &self.readback, 0, (OUT_WORDS * 4) as u64);
        self.queue.submit([enc.finish()]);

        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data: Vec<u32> = match slice.get_mapped_range() {
            Ok(view) => bytemuck::cast_slice(&view).to_vec(),
            Err(_) => return vec![],
        };
        self.readback.unmap();
        let n = (data[0] as usize).min(OUT_WORDS - 1);
        data[1..1 + n].to_vec()
    }
}

pub fn spawn(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("gpu-miner".into())
        .spawn(move || worker(shared))
        .expect("spawn gpu miner");
}

fn set_status(shared: &Shared, s: GpuStatus) {
    *shared.gpu_status.lock().unwrap() = s;
}

fn worker(shared: Arc<Shared>) {
    let mut gpu: Option<GpuMiner> = None;
    let mut failed = false;
    let mut batch: u32 = WG * 1024;
    'outer: loop {
        if shared.paused.load(Relaxed) || !shared.gpu_enabled.load(Relaxed) {
            if !failed {
                set_status(&shared, if gpu.is_some() { GpuStatus::Paused } else { GpuStatus::Off });
            }
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        if gpu.is_none() {
            if failed {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            set_status(&shared, GpuStatus::Starting);
            match GpuMiner::new() {
                Ok(g) => {
                    shared.log(format!("GPU: {}", g.name));
                    gpu = Some(g);
                }
                Err(e) => {
                    shared.log(fill(t().log_gpu_unavailable, &[&e]));
                    set_status(&shared, GpuStatus::Unavailable(e.to_string()));
                    failed = true;
                    continue;
                }
            }
        }
        let g = gpu.as_ref().unwrap();
        let Some((work, generation)) = shared.next_work() else {
            set_status(&shared, GpuStatus::Waiting);
            std::thread::sleep(Duration::from_millis(250));
            continue;
        };
        set_status(&shared, GpuStatus::Mining(g.name.clone()));
        let mut base: u64 = 0;
        while base < (1u64 << 32) {
            let count = batch.min(((1u64 << 32) - base) as u32 / WG * WG).max(WG);
            let t0 = Instant::now();
            for nonce in g.run(&work, base as u32, count) {
                let (_, diff) = verify(&work, nonce);
                shared.report_candidate(&work, nonce, diff);
            }
            let dt = t0.elapsed();
            shared.hashes_gpu.fetch_add(count as u64, Relaxed);
            base += count as u64;

            // adapt batch size so one dispatch takes ~TARGET_DISPATCH
            let ratio = (TARGET_DISPATCH / dt.as_secs_f64().max(1e-4)).clamp(0.5, 2.0);
            batch = (((batch as f64 * ratio) as u32) / WG * WG).clamp(WG * 64, WG * MAX_WG);

            if shared.job_gen.load(Acquire) != generation
                || shared.paused.load(Relaxed)
                || !shared.gpu_enabled.load(Relaxed)
            {
                continue 'outer;
            }
            let usage = shared.gpu_usage.load(Relaxed).clamp(1, 100);
            if usage < 100 {
                std::thread::sleep(dt * (100 - usage) / usage);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::{IV, WorkUnit};
    use sha2::block_api::compress256;

    /// Genesis header: the GPU must find the known nonce.
    #[test]
    fn gpu_finds_genesis_nonce() {
        let Ok(g) = GpuMiner::new() else {
            eprintln!("no GPU, skipping");
            return;
        };
        let raw = hex::decode("0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c").unwrap();
        let mut header = [0u8; 80];
        header.copy_from_slice(&raw);
        let mut mid = IV;
        compress256(&mut mid, &[header[..64].try_into().unwrap()]);
        let w = WorkUnit {
            job_id: String::new(),
            extranonce2: vec![],
            ntime: 0,
            header,
            midstate: mid,
            pool_diff: 1.0,
            net_diff: 1.0,
        };
        let nonce = 0x7c2bac1d_u32;
        let base = nonce - 1000 * 256;
        let found = g.run(&w, base, 4096 * 256);
        assert!(found.contains(&nonce), "found {found:x?}");
        let t = Instant::now();
        let n = 32u32 * 65535 * 256 / 32;
        let _ = g.run(&w, 0, n);
        eprintln!("{} -> {:.1} MH/s", g.name, n as f64 / t.elapsed().as_secs_f64() / 1e6);
    }
}
