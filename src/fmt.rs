//! Number formatting in the NerdMiner style.

pub fn si(v: f64) -> String {
    const UNITS: [&str; 7] = ["", "K", "M", "G", "T", "P", "E"];
    let mut v = v;
    let mut i = 0;
    while v.abs() >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0}")
    } else {
        format!("{v:.2}{}", UNITS[i])
    }
}

pub fn diff(d: f64) -> String {
    if d < 1.0 { format!("{d:.4}") } else { si(d) }
}

/// Hashrate split into value and unit, e.g. ("12.34", "MH/s").
pub fn hashrate(h: f64) -> (String, &'static str) {
    const UNITS: [&str; 6] = ["H/s", "KH/s", "MH/s", "GH/s", "TH/s", "PH/s"];
    let mut v = h;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    let s = if v >= 100.0 { format!("{v:.1}") } else { format!("{v:.2}") };
    (s, UNITS[i])
}
