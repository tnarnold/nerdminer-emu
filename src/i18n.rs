//! User-facing strings.
//!
//! Every language is a `Tr` value, so a missing translation is a compile error.
//! To add a language: add a variant to `Lang`, a `static XX: Tr`, and map it in
//! `Lang::tr`, `Lang::ALL` and `detect`.
//!
//! Templates use `{}` placeholders, filled in order by [`fill`].

use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Pt,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Pt, Lang::En];

    pub fn code(self) -> &'static str {
        match self {
            Lang::Pt => "pt-BR",
            Lang::En => "en",
        }
    }
    /// Name of the language in itself.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::Pt => "Português (Brasil)",
            Lang::En => "English",
        }
    }
    pub fn tr(self) -> &'static Tr {
        match self {
            Lang::Pt => &PT,
            Lang::En => &EN,
        }
    }
    pub fn from_code(c: &str) -> Option<Lang> {
        let c = c.trim().to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| {
            let code = l.code().to_ascii_lowercase();
            c == code || c.split(['-', '_']).next() == code.split('-').next()
        })
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Language from the config value: "auto" (or anything unknown) follows the system.
pub fn resolve(setting: &str) -> Lang {
    if setting.trim().eq_ignore_ascii_case("auto") {
        detect()
    } else {
        Lang::from_code(setting).unwrap_or_else(detect)
    }
}

pub fn detect() -> Lang {
    sys_locale::get_locale().map(|l| from_locale(&l)).unwrap_or(Lang::En)
}

pub fn from_locale(locale: &str) -> Lang {
    match locale.split(['-', '_', '.']).next().unwrap_or("").to_ascii_lowercase().as_str() {
        "pt" => Lang::Pt,
        _ => Lang::En,
    }
}

pub fn set(l: Lang) {
    CURRENT.store(l as u8, Relaxed);
}

pub fn lang() -> Lang {
    match CURRENT.load(Relaxed) {
        0 => Lang::Pt,
        _ => Lang::En,
    }
}

/// Strings of the current language.
pub fn t() -> &'static Tr {
    lang().tr()
}

/// Replace each `{}` in `tpl` with the next argument.
pub fn fill(tpl: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(tpl.len() + 16);
    let mut it = args.iter();
    let mut rest = tpl;
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        if let Some(a) = it.next() {
            out.push_str(&a.to_string());
        }
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

pub struct Tr {
    // ---- number formatting
    pub thousands_sep: char,
    pub decimal_sep: char,
    /// Suffixes for 1e3, 1e6, 1e9, 1e12 (pt: " mil", " mi"...; en: "k", "M"...).
    pub big_units: [&'static str; 4],
    pub days: &'static str,
    pub years: &'static str,
    pub one_in: &'static str,

    // ---- window / toolbar
    pub hover_screen: &'static str,
    pub prev_screen: &'static str,
    pub next_screen: &'static str,
    pub btn_mine: &'static str,
    pub btn_pause: &'static str,
    pub settings: &'static str,
    pub always_on_top: &'static str,
    pub hide: &'static str,
    pub hide_tip: &'static str,
    pub quit: &'static str,
    pub quit_tip: &'static str,
    pub paused_screen: &'static str,

    // ---- settings
    pub btc_address: &'static str,
    pub pool: &'static str,
    pub port: &'static str,
    pub worker: &'static str,
    pub password: &'static str,
    pub suggest_diff: &'static str,
    pub suggest_diff_tip: &'static str,
    pub save_reconnect: &'static str,
    pub h_mining: &'static str,
    pub device: &'static str,
    pub threads: &'static str,
    pub cpu_usage_slider: &'static str,
    /// {pct} {cores}
    pub cpu_total: &'static str,
    pub gpu_usage_slider: &'static str,
    pub status: &'static str,
    pub h_window: &'static str,
    pub start_hidden: &'static str,
    pub zoom: &'static str,
    pub currency: &'static str,
    pub currency_tip: &'static str,
    pub language: &'static str,
    pub language_auto: &'static str,
    pub h_log: &'static str,
    pub lottery_note: &'static str,
    /// In the order of `currency::CURRENCIES`.
    pub currency_names: [&'static str; 8],

    // ---- pool / gpu status and log
    pub st_mining: &'static str,
    pub st_connecting: &'static str,
    pub st_offline: &'static str,
    pub st_paused: &'static str,
    pub set_address: &'static str,
    pub gpu_off: &'static str,
    pub gpu_paused: &'static str,
    pub gpu_init: &'static str,
    pub gpu_unavailable: &'static str,
    pub gpu_waiting: &'static str,
    pub gpu_mining: &'static str,
    pub gpu_none: &'static str,
    pub log_gpu_unavailable: &'static str,
    pub log_tray_unavailable: &'static str,
    pub err_config_changed: &'static str,
    pub err_closed: &'static str,
    pub err_no_response: &'static str,
    pub err_reconnect: &'static str,
    pub err_auth: &'static str,
    pub err_bad_address: &'static str,
    pub log_share_sent: &'static str,
    pub log_pool_diff: &'static str,
    pub log_authorized: &'static str,
    pub log_share_ok: &'static str,
    pub log_share_rejected: &'static str,
    pub log_best: &'static str,
    pub log_block: &'static str,

    // ---- tray menu
    pub pause_mining: &'static str,
    pub resume_mining: &'static str,
    pub open_pool: &'static str,
    pub open_mempool: &'static str,
    pub open_lab: &'static str,
    pub hide_window: &'static str,
    pub show_window: &'static str,
    pub sub_device: &'static str,
    pub sub_cpu: &'static str,
    pub sub_gpu: &'static str,
    pub sub_tx: &'static str,
    pub sub_currency: &'static str,
    pub sub_fields: &'static str,
    pub sub_language: &'static str,
    pub cpu_step: &'static str,
    /// In the order of `tray::TX_SIZES`.
    pub tx_labels: [&'static str; 3],
    /// In the order of `tray::FIELDS`.
    pub field_labels: [&'static str; 5],
    pub r_no_device: &'static str,
    pub r_best: &'static str,
    /// {shares} {accepted} {blocks}
    pub r_shares: &'static str,
    pub r_chance: &'static str,
    pub r_block_every: &'static str,
    pub r_block: &'static str,
    pub ago: &'static str,
    /// {progress} {change} {time} {blocks}
    pub r_diff_adj: &'static str,
    pub r_network: &'static str,
    pub loading: &'static str,
    pub r_occupancy: &'static str,
    pub block_one: &'static str,
    pub block_many: &'static str,
    /// {fee}
    pub occ_fee: &'static str,
    /// {amount} {block(s)}
    pub paying_queue: &'static str,
    pub mostly_below: &'static str,
    pub r_next_block: &'static str,
    pub median: &'static str,
    pub fee_title: &'static str,
    pub recommended: &'static str,
    /// Next block, ~30 min, ~1 h, no rush.
    pub tiers: [&'static str; 4],
    /// Free, moderate, congested, severe.
    pub congestion: [&'static str; 4],
    pub recommendation: [&'static str; 4],
    pub tip_block: &'static str,
    pub tip_network: &'static str,
}

pub static PT: Tr = Tr {
    thousands_sep: '.',
    decimal_sep: ',',
    big_units: [" mil", " mi", " bi", " tri"],
    days: "dias",
    years: "anos",
    one_in: "1 em {}",

    hover_screen: "Clique: próxima tela • Arraste: mover",
    prev_screen: "Tela anterior",
    next_screen: "Próxima tela",
    btn_mine: "▶ Minerar",
    btn_pause: "⏸ Pausar",
    settings: "Configurações",
    always_on_top: "Sempre no topo",
    hide: "Esconder",
    hide_tip: "Esconder na bandeja do sistema",
    quit: "Sair",
    quit_tip: "Encerrar o minerador",
    paused_screen: "PAUSADO",

    btc_address: "Endereço BTC",
    pool: "Pool",
    port: "Porta",
    worker: "Worker",
    password: "Senha",
    suggest_diff: "Dif. sugerida",
    suggest_diff_tip: "Enviada via mining.suggest_difficulty (0 = não enviar)",
    save_reconnect: "Salvar e reconectar",
    h_mining: "Mineração",
    device: "Dispositivo",
    threads: "threads",
    cpu_usage_slider: "% uso por thread",
    cpu_total: "≈ {}% da CPU total ({} núcleos lógicos)",
    gpu_usage_slider: "% uso da GPU",
    status: "Status: {}",
    h_window: "Janela",
    start_hidden: "Iniciar escondido na bandeja",
    zoom: "Zoom da tela",
    currency: "Moeda",
    currency_tip: "Preço do BTC nas telas e na bandeja, e custo das transações",
    language: "Idioma",
    language_auto: "Automático (sistema)",
    h_log: "Log",
    lottery_note: "Mineração solo é uma loteria: a chance de achar um bloco com CPU/GPU é minúscula.",
    currency_names: [
        "Real", "Dólar americano", "Euro", "Libra esterlina", "Dólar canadense", "Franco suíço",
        "Dólar australiano", "Iene",
    ],

    st_mining: "Minerando",
    st_connecting: "Conectando",
    st_offline: "Sem pool",
    st_paused: "Pausado",
    set_address: "configure seu endereço BTC",
    gpu_off: "Desligada",
    gpu_paused: "Pausada",
    gpu_init: "Inicializando…",
    gpu_unavailable: "Indisponível: {}",
    gpu_waiting: "Aguardando trabalho",
    gpu_mining: "Minerando — {}",
    gpu_none: "nenhuma GPU: {}",
    log_gpu_unavailable: "GPU indisponível: {}",
    log_tray_unavailable: "Bandeja indisponível: {}",
    err_config_changed: "configuração alterada, reconectando",
    err_closed: "conexão encerrada pela pool",
    err_no_response: "sem resposta da pool",
    err_reconnect: "pool pediu reconexão",
    err_auth: "autorização recusada: {}",
    err_bad_address: "endereço da pool inválido",
    log_share_sent: "Share enviado (dif {})",
    log_pool_diff: "Dificuldade da pool: {}",
    log_authorized: "Autorizado como {}",
    log_share_ok: "Share ACEITO (dif {})",
    log_share_rejected: "Share rejeitado: {}",
    log_best: "Nova melhor dificuldade: {}",
    log_block: "!!! BLOCO VÁLIDO ENCONTRADO !!!",

    pause_mining: "Pausar mineração",
    resume_mining: "Retomar mineração",
    open_pool: "Ver na pool…",
    open_mempool: "Ver no mempool.space…",
    open_lab: "🎓 Como funciona a mineração…",
    hide_window: "Ocultar janela",
    show_window: "Mostrar janela",
    sub_device: "Dispositivo",
    sub_cpu: "Uso da CPU",
    sub_gpu: "Uso da GPU",
    sub_tx: "Tamanho da transação",
    sub_currency: "Moeda",
    sub_fields: "Barra de menus mostra",
    sub_language: "Idioma",
    cpu_step: "{}% por thread",
    tx_labels: ["Simples 1→2, SegWit · 141 vB", "Taproot 1→2 · 154 vB", "Consolidação 5→1 · 382 vB"],
    field_labels: [
        "Hashrate",
        "Taxa recomendada + ocupação",
        "Tempo desde o último bloco",
        "Melhor dificuldade",
        "Preço do BTC",
    ],
    r_no_device: "nenhum dispositivo",
    r_best: "Melhor dificuldade",
    r_shares: "Shares   {} (32 bits) · aceitas {} · blocos {}",
    r_chance: "Chance hoje",
    r_block_every: "bloco a cada ~{}",
    r_block: "Bloco",
    ago: "há {}",
    r_diff_adj: "Ajuste de dificuldade   {}% · {}% em ~{} ({} blocos)",
    r_network: "REDE BITCOIN",
    loading: "carregando…",
    r_occupancy: "Ocupação",
    block_one: "bloco",
    block_many: "blocos",
    occ_fee: "taxa p/ ~30 min: {} sat/vB",
    paying_queue: "fila pagante {} {}",
    mostly_below: "(maioria abaixo de {} sat/vB)",
    r_next_block: "Próximo bloco",
    median: "mediana",
    fee_title: "TAXA PARA ENVIAR AGORA",
    recommended: "✅ Recomendado",
    tiers: ["🚀 Próximo bloco (~10 min)", "⏱ ~30 min", "🕐 ~1 hora", "🐢 Sem pressa"],
    congestion: ["Livre", "Moderada", "Congestionada", "Muito congestionada"],
    recommendation: [
        "rede livre, não pague mais",
        "deve confirmar em ~30 min",
        "taxa baixa pode demorar horas",
        "rede cheia: envie só se for urgente",
    ],
    tip_block: "Bloco #{} · há {}",
    tip_network: "Rede {} {} · enviar: {} sat/vB {}",
};

pub static EN: Tr = Tr {
    thousands_sep: ',',
    decimal_sep: '.',
    big_units: ["k", "M", "B", "T"],
    days: "days",
    years: "years",
    one_in: "1 in {}",

    hover_screen: "Click: next screen • Drag: move",
    prev_screen: "Previous screen",
    next_screen: "Next screen",
    btn_mine: "▶ Mine",
    btn_pause: "⏸ Pause",
    settings: "Settings",
    always_on_top: "Always on top",
    hide: "Hide",
    hide_tip: "Hide to the system tray",
    quit: "Quit",
    quit_tip: "Stop the miner and quit",
    paused_screen: "PAUSED",

    btc_address: "BTC address",
    pool: "Pool",
    port: "Port",
    worker: "Worker",
    password: "Password",
    suggest_diff: "Suggested diff.",
    suggest_diff_tip: "Sent via mining.suggest_difficulty (0 = don't send)",
    save_reconnect: "Save and reconnect",
    h_mining: "Mining",
    device: "Device",
    threads: "threads",
    cpu_usage_slider: "% usage per thread",
    cpu_total: "≈ {}% of total CPU ({} logical cores)",
    gpu_usage_slider: "% GPU usage",
    status: "Status: {}",
    h_window: "Window",
    start_hidden: "Start hidden in the tray",
    zoom: "Screen zoom",
    currency: "Currency",
    currency_tip: "BTC price on the screens and in the tray, and transaction costs",
    language: "Language",
    language_auto: "Automatic (system)",
    h_log: "Log",
    lottery_note: "Solo mining is a lottery: the chance of finding a block with a CPU/GPU is tiny.",
    currency_names: [
        "Brazilian real", "US dollar", "Euro", "Pound sterling", "Canadian dollar", "Swiss franc",
        "Australian dollar", "Japanese yen",
    ],

    st_mining: "Mining",
    st_connecting: "Connecting",
    st_offline: "No pool",
    st_paused: "Paused",
    set_address: "set your BTC address",
    gpu_off: "Off",
    gpu_paused: "Paused",
    gpu_init: "Starting…",
    gpu_unavailable: "Unavailable: {}",
    gpu_waiting: "Waiting for work",
    gpu_mining: "Mining — {}",
    gpu_none: "no GPU: {}",
    log_gpu_unavailable: "GPU unavailable: {}",
    log_tray_unavailable: "Tray unavailable: {}",
    err_config_changed: "settings changed, reconnecting",
    err_closed: "connection closed by the pool",
    err_no_response: "no response from the pool",
    err_reconnect: "the pool asked to reconnect",
    err_auth: "authorization refused: {}",
    err_bad_address: "invalid pool address",
    log_share_sent: "Share sent (diff {})",
    log_pool_diff: "Pool difficulty: {}",
    log_authorized: "Authorized as {}",
    log_share_ok: "Share ACCEPTED (diff {})",
    log_share_rejected: "Share rejected: {}",
    log_best: "New best difficulty: {}",
    log_block: "!!! VALID BLOCK FOUND !!!",

    pause_mining: "Pause mining",
    resume_mining: "Resume mining",
    open_pool: "Open pool page…",
    open_mempool: "Open in mempool.space…",
    open_lab: "🎓 How mining works…",
    hide_window: "Hide window",
    show_window: "Show window",
    sub_device: "Device",
    sub_cpu: "CPU usage",
    sub_gpu: "GPU usage",
    sub_tx: "Transaction size",
    sub_currency: "Currency",
    sub_fields: "Menu bar shows",
    sub_language: "Language",
    cpu_step: "{}% per thread",
    tx_labels: ["Simple 1→2, SegWit · 141 vB", "Taproot 1→2 · 154 vB", "Consolidation 5→1 · 382 vB"],
    field_labels: [
        "Hashrate",
        "Recommended fee + occupancy",
        "Time since the last block",
        "Best difficulty",
        "BTC price",
    ],
    r_no_device: "no device",
    r_best: "Best difficulty",
    r_shares: "Shares   {} (32-bit) · accepted {} · blocks {}",
    r_chance: "Odds today",
    r_block_every: "a block every ~{}",
    r_block: "Block",
    ago: "{} ago",
    r_diff_adj: "Difficulty adjustment   {}% · {}% in ~{} ({} blocks)",
    r_network: "BITCOIN NETWORK",
    loading: "loading…",
    r_occupancy: "Occupancy",
    block_one: "block",
    block_many: "blocks",
    occ_fee: "~30 min fee: {} sat/vB",
    paying_queue: "paying queue {} {}",
    mostly_below: "(mostly below {} sat/vB)",
    r_next_block: "Next block",
    median: "median",
    fee_title: "FEE TO SEND NOW",
    recommended: "✅ Recommended",
    tiers: ["🚀 Next block (~10 min)", "⏱ ~30 min", "🕐 ~1 hour", "🐢 No rush"],
    congestion: ["Free", "Moderate", "Congested", "Very congested"],
    recommendation: [
        "network is free, don't pay more",
        "should confirm in ~30 min",
        "low fees may take hours",
        "network is full: send only if urgent",
    ],
    tip_block: "Block #{} · {} ago",
    tip_network: "Network {} {} · send: {} sat/vB {}",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_and_codes() {
        assert_eq!(fill("a {} b {}", &[&1, &"x"]), "a 1 b x");
        assert_eq!(fill("{} extra {}", &[&1]), "1 extra ");
        assert_eq!(Lang::from_code("pt-BR"), Some(Lang::Pt));
        assert_eq!(Lang::from_code("pt_PT"), Some(Lang::Pt));
        assert_eq!(Lang::from_code("en-US"), Some(Lang::En));
        assert_eq!(Lang::from_code("xx"), None);
        assert_eq!(from_locale("pt_BR.UTF-8"), Lang::Pt);
        assert_eq!(from_locale("en_BR"), Lang::En);
        assert_eq!(from_locale("de-DE"), Lang::En);
    }

    #[test]
    fn templates_have_matching_placeholders() {
        let n = |s: &str| s.matches("{}").count();
        for l in Lang::ALL {
            let t = l.tr();
            for (s, k) in [
                (t.one_in, 1), (t.cpu_total, 2), (t.status, 1), (t.gpu_unavailable, 1), (t.gpu_mining, 1),
                (t.gpu_none, 1), (t.log_gpu_unavailable, 1), (t.log_tray_unavailable, 1), (t.err_auth, 1),
                (t.log_share_sent, 1), (t.log_pool_diff, 1), (t.log_authorized, 1), (t.log_share_ok, 1),
                (t.log_share_rejected, 1), (t.log_best, 1), (t.cpu_step, 1), (t.r_shares, 3),
                (t.r_block_every, 1), (t.ago, 1), (t.r_diff_adj, 4), (t.occ_fee, 1), (t.paying_queue, 2), (t.mostly_below, 1),
                (t.tip_block, 2), (t.tip_network, 4),
            ] {
                assert_eq!(n(s), k, "{:?}: {s}", l);
            }
        }
    }
}
