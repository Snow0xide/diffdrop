//! Interface language. The strings live in `locales/*.json`, shared with the
//! frontend; the backend needs them for the native menu and dialog titles.

use std::collections::HashMap;
use std::sync::OnceLock;

const EN: &str = include_str!("../../locales/en.json");
const IT: &str = include_str!("../../locales/it.json");

struct Strings {
    lang: &'static str,
    table: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

static STRINGS: OnceLock<Strings> = OnceLock::new();

fn parse(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).expect("locale files are valid JSON")
}

fn strings() -> &'static Strings {
    STRINGS.get_or_init(|| {
        let system = sys_locale::get_locale().unwrap_or_default().to_lowercase();
        let lang = if system.starts_with("it") { "it" } else { "en" };
        let table = parse(if lang == "it" { IT } else { EN });
        Strings {
            lang,
            table,
            fallback: parse(EN),
        }
    })
}

/// `"it"` or `"en"`.
pub fn lang() -> &'static str {
    strings().lang
}

pub fn t(key: &str) -> String {
    let s = strings();
    s.table
        .get(key)
        .or_else(|| s.fallback.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}
