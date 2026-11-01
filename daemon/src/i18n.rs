use rust_embed::RustEmbed;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(RustEmbed)]
#[folder = "locales/"]
pub struct LocalesEmbed;

static CURRENT_LOCALE: RwLock<Option<I18n>> = RwLock::new(None);

#[derive(Debug, Clone)]
pub struct I18n {
    translations: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl I18n {
    pub fn new(lang: &str) -> Self {
        let fallback = Self::load_map("en");
        let translations = if lang == "en" {
            fallback.clone()
        } else {
            Self::load_map(lang)
        };

        Self {
            translations,
            fallback,
        }
    }

    fn load_map(lang: &str) -> HashMap<String, String> {
        let filename = format!("{}.json", lang);
        if let Some(file) = LocalesEmbed::get(&filename) {
            if let Ok(val) = serde_json::from_slice::<Value>(&file.data) {
                if let Some(obj) = val.as_object() {
                    let mut map = HashMap::new();
                    for (k, v) in obj {
                        if let Some(s) = v.as_str() {
                            map.insert(k.clone(), s.to_string());
                        }
                    }
                    return map;
                }
            }
        }
        HashMap::new()
    }

    pub fn t(&self, key: &str) -> String {
        if let Some(val) = self.translations.get(key) {
            return val.clone();
        }
        if let Some(val) = self.fallback.get(key) {
            return val.clone();
        }
        key.to_string()
    }

    pub fn t_fmt(&self, key: &str, vars: &[(&str, &str)]) -> String {
        let mut text = self.t(key);
        for (k, v) in vars {
            let placeholder = format!("{{{}}}", k);
            text = text.replace(&placeholder, v);
        }
        text
    }

    pub fn detect_system_language() -> String {
        if let Some(locale) = sys_locale::get_locale() {
            let lower = locale.to_lowercase();
            if lower.starts_with("ru") || lower.starts_with("be") || lower.starts_with("uk") {
                return "ru".to_string();
            }
        }
        "en".to_string()
    }

    pub fn available_languages() -> Vec<(&'static str, &'static str)> {
        vec![
            ("en", "English"),
            ("ru", "Русский"),
        ]
    }
}

pub fn init_i18n(lang: &str) {
    if let Ok(mut lock) = CURRENT_LOCALE.write() {
        *lock = Some(I18n::new(lang));
    }
}

pub fn t(key: &str) -> String {
    if let Ok(lock) = CURRENT_LOCALE.read() {
        if let Some(ref i18n) = *lock {
            return i18n.t(key);
        }
    }
    let fallback = I18n::new(&I18n::detect_system_language());
    fallback.t(key)
}

pub fn t_fmt(key: &str, vars: &[(&str, &str)]) -> String {
    if let Ok(lock) = CURRENT_LOCALE.read() {
        if let Some(ref i18n) = *lock {
            return i18n.t_fmt(key, vars);
        }
    }
    let fallback = I18n::new(&I18n::detect_system_language());
    fallback.t(key)
}