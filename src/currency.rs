//! Display currency for the BTC price and fee costs.
//!
//! mempool.space provides USD, EUR, GBP, CAD, CHF, AUD and JPY; BRL comes from Coinbase
//! (fallback Binance), see `netinfo`.

use crate::fmt;

/// (ISO code, symbol). Names are translated in `i18n::Tr::currency_names` (same order).
pub const CURRENCIES: [(&str, &str); 8] = [
    ("BRL", "R$"),
    ("USD", "US$"),
    ("EUR", "€"),
    ("GBP", "£"),
    ("CAD", "C$"),
    ("CHF", "CHF"),
    ("AUD", "A$"),
    ("JPY", "¥"),
];

/// Currency name in the current language.
pub fn name(c: &str) -> &'static str {
    let c = code(c);
    let i = CURRENCIES.iter().position(|x| x.0 == c).unwrap_or(1);
    crate::i18n::t().currency_names[i]
}

/// Normalized code: one of `CURRENCIES`, USD for anything unknown.
pub fn code(c: &str) -> &'static str {
    CURRENCIES.iter().find(|x| x.0.eq_ignore_ascii_case(c.trim())).map(|x| x.0).unwrap_or("USD")
}

pub fn symbol(c: &str) -> &'static str {
    let c = code(c);
    CURRENCIES.iter().find(|x| x.0 == c).map(|x| x.1).unwrap_or("US$")
}

/// Symbol on the NerdMiner screens: the firmware shows "$" for dollars.
pub fn screen_symbol(c: &str) -> &'static str {
    match code(c) {
        "USD" => "$",
        other => symbol(other),
    }
}

fn spaced(sym: &str) -> String {
    // "R$ 1,20", "CHF 1,20", but "€1,20", "£1,20", "¥120"
    if sym.chars().count() > 1 { format!("{sym} ") } else { sym.to_string() }
}

// Number separators follow the current language (tests run with the default, Portuguese).

/// Small amounts (fee costs): "R$ 1,21", "€0,20", "¥34".
pub fn amount(v: f64, c: &str) -> String {
    let dec = if code(c) == "JPY" { 0 } else { 2 };
    format!("{}{}", spaced(symbol(c)), fmt::num(v, dec))
}

/// BTC price in full: "R$ 430.462", "US$ 78.454".
pub fn price(v: f64, c: &str) -> String {
    format!("{}{}", spaced(symbol(c)), fmt::num(v, 0))
}

/// BTC price, compact for the menu bar: "R$ 430k", "US$ 78,5k", "¥12,4M".
pub fn price_short(v: f64, c: &str) -> String {
    let n = if v >= 1e6 {
        format!("{}M", fmt::num(v / 1e6, 1))
    } else if v >= 100_000.0 {
        format!("{}k", fmt::num(v / 1e3, 0))
    } else {
        format!("{}k", fmt::num(v / 1e3, 1))
    };
    format!("{}{n}", spaced(symbol(c)))
}

/// BTC price on the NerdMiner screens, firmware style (no separators): "$78454", "R$430462".
pub fn price_screen(v: f64, c: &str) -> String {
    format!("{}{}", screen_symbol(c), v.round() as u64)
}

/// Default from the system locale: pt-BR -> BRL, en-GB -> GBP, de-DE -> EUR, ...
pub fn default_for_system() -> String {
    sys_locale::get_locale().map(|l| from_locale(&l)).unwrap_or("USD").to_string()
}

pub fn from_locale(locale: &str) -> &'static str {
    let l = locale.replace('_', "-");
    let mut parts = l.split(['-', '.', '@']);
    let lang = parts.next().unwrap_or("").to_ascii_lowercase();
    // Region is the first 2-letter uppercase-able subtag after the language (skips scripts like "Hans").
    let region = parts.find(|p| p.len() == 2).map(|p| p.to_ascii_uppercase());
    const EURO: [&str; 20] = [
        "AT", "BE", "CY", "DE", "EE", "ES", "FI", "FR", "GR", "HR", "IE", "IT", "LT", "LU", "LV", "MT", "NL", "PT",
        "SI", "SK",
    ];
    match region.as_deref() {
        Some("BR") => "BRL",
        Some("US") => "USD",
        Some("GB") => "GBP",
        Some("CA") => "CAD",
        Some("CH") | Some("LI") => "CHF",
        Some("AU") => "AUD",
        Some("JP") => "JPY",
        Some(r) if EURO.contains(&r) => "EUR",
        Some(_) => "USD",
        None => match lang.as_str() {
            "pt" => "BRL",
            "ja" => "JPY",
            "de" | "fr" | "es" | "it" | "nl" | "fi" | "el" | "et" | "lv" | "lt" | "sk" | "sl" => "EUR",
            _ => "USD",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_mapping() {
        assert_eq!(from_locale("pt-BR"), "BRL");
        assert_eq!(from_locale("pt_BR.UTF-8"), "BRL");
        assert_eq!(from_locale("pt"), "BRL");
        assert_eq!(from_locale("pt-PT"), "EUR");
        assert_eq!(from_locale("en-US"), "USD");
        assert_eq!(from_locale("en-GB"), "GBP");
        assert_eq!(from_locale("de-CH"), "CHF");
        assert_eq!(from_locale("zh-Hans-CN"), "USD");
        assert_eq!(from_locale("ja-JP"), "JPY");
        assert_eq!(from_locale("en"), "USD");
    }

    #[test]
    fn formats() {
        assert_eq!(amount(1.2134, "BRL"), "R$ 1,21");
        assert_eq!(amount(0.2, "eur"), "€0,20");
        assert_eq!(amount(34.4, "JPY"), "¥34");
        assert_eq!(price(430462.4, "BRL"), "R$ 430.462");
        assert_eq!(price_short(430462.0, "BRL"), "R$ 430k");
        assert_eq!(price_short(78454.0, "USD"), "US$ 78,5k");
        assert_eq!(price_short(12_396_188.0, "JPY"), "¥12,4M");
        assert_eq!(price_screen(78454.4, "USD"), "$78454");
        assert_eq!(price_screen(430462.0, "BRL"), "R$430462");
        assert_eq!(code("xyz"), "USD");
    }
}
