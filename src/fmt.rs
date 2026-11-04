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

// ------------------------------------------------------------------ localized formatting (tray menu)
//
// Each function has a `*_t` variant taking the language explicitly (used by tests);
// the plain one uses the current language.

use crate::i18n::{self, Tr};

/// Number with `dec` decimals and the language's separators: pt "78.381,50", en "78,381.50".
pub fn num(v: f64, dec: usize) -> String {
    num_t(v, dec, i18n::t())
}

pub fn num_t(v: f64, dec: usize, t: &Tr) -> String {
    let s = format!("{:.*}", dec, v.abs());
    let (int, frac) = s.split_once('.').map(|(a, b)| (a, Some(b))).unwrap_or((&s, None));
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(t.thousands_sep);
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push(t.decimal_sep);
        out.push_str(f);
    }
    if v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.insert(0, '−');
    }
    out
}

/// `num` with one decimal, dropping a trailing zero ("5,0" -> "5").
fn num_trim(v: f64, t: &Tr) -> String {
    let s = num_t(v, 1, t);
    let zero = format!("{}0", t.decimal_sep);
    s.strip_suffix(&zero).map(str::to_string).unwrap_or(s)
}

/// Large number in words: pt "47,3 mil" / "92,2 bi", en "47.3k" / "92.2B".
pub fn big(n: f64) -> String {
    big_t(n, i18n::t())
}

pub fn big_t(n: f64, t: &Tr) -> String {
    for (lim, unit) in [1e12, 1e9, 1e6, 1e3].into_iter().zip(t.big_units.iter().rev()) {
        if n >= lim {
            let v = n / lim;
            let s = if v < 100.0 { num_trim(v, t) } else { num_t(v, 0, t) };
            return format!("{s}{unit}");
        }
    }
    num_t(n, 0, t)
}

/// Short duration: "45 s", "12 min", "3 h 05", "8,8 dias" / "8.8 days", "252 mi anos" / "252M years".
pub fn duration(secs: f64) -> String {
    duration_t(secs, i18n::t())
}

pub fn duration_t(secs: f64, t: &Tr) -> String {
    let secs = secs.max(0.0);
    if secs < 60.0 {
        format!("{:.0} s", secs)
    } else if secs < 3600.0 {
        format!("{:.0} min", (secs / 60.0).floor())
    } else if secs < 86400.0 * 2.0 {
        let m = (secs / 60.0).floor() as u64;
        format!("{} h {:02}", m / 60, m % 60)
    } else if secs < 86400.0 * 365.0 * 2.0 {
        format!("{} {}", num_trim(secs / 86400.0, t), t.days)
    } else {
        format!("{} {}", big_t(secs / 31_557_600.0, t), t.years)
    }
}

/// Probability as "12,5%" or "1 em 92 bi" / "1 in 92B".
pub fn odds(p: f64) -> String {
    odds_t(p, i18n::t())
}

pub fn odds_t(p: f64, t: &Tr) -> String {
    if p >= 0.995 {
        "≈ 100%".into()
    } else if p >= 0.01 {
        format!("{}%", num_t(p * 100.0, 1, t))
    } else if p > 0.0 {
        i18n::fill(t.one_in, &[&big_t(1.0 / p, t)])
    } else {
        "—".into()
    }
}

/// Fee rate: integers without decimals, sub-sat rates with up to 2.
pub fn feerate(f: f64) -> String {
    feerate_t(f, i18n::t())
}

pub fn feerate_t(f: f64, t: &Tr) -> String {
    if f >= 10.0 || (f - f.round()).abs() < 0.05 {
        num_t(f.round(), 0, t)
    } else {
        num_t(f, if f < 1.0 { 2 } else { 1 }, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{EN, PT};

    #[test]
    fn pt_br_numbers() {
        let t = &PT;
        assert_eq!(num_t(78381.0, 0, t), "78.381");
        assert_eq!(num_t(0.2219, 2, t), "0,22");
        assert_eq!(num_t(-2.66, 2, t), "−2,66");
        assert_eq!(num_t(1234567.891, 1, t), "1.234.567,9");
        assert_eq!(big_t(47_307.0, t), "47,3 mil");
        assert_eq!(big_t(92.19e9, t), "92,2 bi");
        assert_eq!(big_t(252.4e6, t), "252 mi");
        assert_eq!(big_t(5.0e6, t), "5 mi");
        assert_eq!(odds_t(1.0 / 92.19e9, t), "1 em 92,2 bi");
        assert_eq!(odds_t(0.125, t), "12,5%");
        assert_eq!(duration_t(125.0, t), "2 min");
        assert_eq!(duration_t(3.0 * 3600.0 + 300.0, t), "3 h 05");
        assert_eq!(duration_t(757_904.0, t), "8,8 dias");
        assert_eq!(feerate_t(2.0, t), "2");
        assert_eq!(feerate_t(1.32, t), "1,3");
        assert_eq!(feerate_t(0.12, t), "0,12");
    }

    #[test]
    fn english_numbers() {
        let t = &EN;
        assert_eq!(num_t(78381.0, 0, t), "78,381");
        assert_eq!(num_t(1234567.891, 1, t), "1,234,567.9");
        assert_eq!(big_t(47_307.0, t), "47.3k");
        assert_eq!(big_t(92.19e9, t), "92.2B");
        assert_eq!(odds_t(1.0 / 92.19e9, t), "1 in 92.2B");
        assert_eq!(duration_t(757_904.0, t), "8.8 days");
        assert_eq!(duration_t(252.4e6 * 31_557_600.0, t), "252M years");
        assert_eq!(feerate_t(1.32, t), "1.3");
    }
}
