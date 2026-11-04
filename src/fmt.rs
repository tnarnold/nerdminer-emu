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

// ------------------------------------------------------------------ pt-BR formatting (tray menu)

/// Number with `dec` decimals, "." for thousands and "," for decimals: 78381.5 -> "78.381,50".
pub fn br(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v.abs());
    let (int, frac) = s.split_once('.').map(|(a, b)| (a, Some(b))).unwrap_or((&s, None));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push(',');
        out.push_str(f);
    }
    if v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.insert(0, '−');
    }
    out
}

/// Large number in words: 47300 -> "47,3 mil", 9.2e10 -> "92 bi".
pub fn br_big(n: f64) -> String {
    for (lim, unit) in [(1e12, "tri"), (1e9, "bi"), (1e6, "mi"), (1e3, "mil")] {
        if n >= lim {
            let v = n / lim;
            return format!("{} {unit}", br(v, if v < 100.0 { 1 } else { 0 }).replace(",0", ""));
        }
    }
    br(n, 0)
}

/// Short duration: "45 s", "12 min", "3 h 05", "8,8 dias", "252 mi anos".
pub fn duration(secs: f64) -> String {
    let secs = secs.max(0.0);
    if secs < 60.0 {
        format!("{:.0} s", secs)
    } else if secs < 3600.0 {
        format!("{:.0} min", (secs / 60.0).floor())
    } else if secs < 86400.0 * 2.0 {
        let m = (secs / 60.0).floor() as u64;
        format!("{} h {:02}", m / 60, m % 60)
    } else if secs < 86400.0 * 365.0 * 2.0 {
        format!("{} dias", br(secs / 86400.0, 1).replace(",0", ""))
    } else {
        format!("{} anos", br_big(secs / 31_557_600.0))
    }
}

/// Probability as "12,5%" or "1 em 92 bi".
pub fn odds(p: f64) -> String {
    if p >= 0.995 {
        "≈ 100%".into()
    } else if p >= 0.01 {
        format!("{}%", br(p * 100.0, 1))
    } else if p > 0.0 {
        format!("1 em {}", br_big(1.0 / p))
    } else {
        "—".into()
    }
}

/// Fee rate: integers without decimals, sub-sat rates with up to 2.
pub fn feerate(f: f64) -> String {
    if f >= 10.0 || (f - f.round()).abs() < 0.05 {
        format!("{}", br(f.round(), 0))
    } else {
        br(f, if f < 1.0 { 2 } else { 1 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pt_br_numbers() {
        assert_eq!(br(78381.0, 0), "78.381");
        assert_eq!(br(0.2219, 2), "0,22");
        assert_eq!(br(-2.66, 2), "−2,66");
        assert_eq!(br(1234567.891, 1), "1.234.567,9");
        assert_eq!(br_big(47_307.0), "47,3 mil");
        assert_eq!(br_big(92.19e9), "92,2 bi");
        assert_eq!(br_big(252.4e6), "252 mi");
        assert_eq!(br_big(5.0e6), "5 mi");
        assert_eq!(odds(1.0 / 92.19e9), "1 em 92,2 bi");
        assert_eq!(odds(0.125), "12,5%");
        assert_eq!(duration(125.0), "2 min");
        assert_eq!(duration(3.0 * 3600.0 + 300.0), "3 h 05");
        assert_eq!(duration(757_904.0), "8,8 dias");
        assert_eq!(feerate(2.0), "2");
        assert_eq!(feerate(1.32), "1,3");
        assert_eq!(feerate(0.12), "0,12");
    }
}
