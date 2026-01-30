//! Internationalization (i18n) support

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Internationalization handler
pub struct I18n {
    /// Current language
    language: String,
    /// Language data: lang -> key -> translation
    translations: HashMap<String, HashMap<String, serde_yaml::Value>>,
}

impl I18n {
    /// Create a new i18n handler
    pub fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
            translations: HashMap::new(),
        }
    }

    /// Load language files from a directory
    pub fn load_languages<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());
                if matches!(ext, Some("yml") | Some("yaml") | Some("json")) {
                    let lang = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("en")
                        .to_string();

                    let content = fs::read_to_string(&path)?;

                    // Try to parse, skip invalid files
                    let data: Option<HashMap<String, serde_yaml::Value>> = if ext == Some("json") {
                        match serde_json::from_str::<serde_json::Value>(&content) {
                            Ok(json) => Some(convert_json_to_yaml(json)),
                            Err(e) => {
                                tracing::warn!("Failed to parse language file {:?}: {}", path, e);
                                None
                            }
                        }
                    } else {
                        match serde_yaml::from_str(&content) {
                            Ok(data) => Some(data),
                            Err(e) => {
                                tracing::warn!("Failed to parse language file {:?}: {}", path, e);
                                None
                            }
                        }
                    };

                    if let Some(data) = data {
                        self.translations.insert(lang, data);
                        tracing::debug!("Loaded language file: {:?}", path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get the current language
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Set the current language
    pub fn set_language(&mut self, lang: &str) {
        self.language = lang.to_string();
    }

    /// Get a translation by key (__ function)
    /// Key can be nested like "menu.home"
    pub fn get(&self, key: &str) -> String {
        self.get_for_lang(&self.language, key)
    }

    /// Get a translation for a specific language
    pub fn get_for_lang(&self, lang: &str, key: &str) -> String {
        if let Some(lang_data) = self.translations.get(lang) {
            if let Some(value) = get_nested_value(lang_data, key) {
                return yaml_value_to_string(value);
            }
        }

        // Fallback to English
        if lang != "en" {
            if let Some(lang_data) = self.translations.get("en") {
                if let Some(value) = get_nested_value(lang_data, key) {
                    return yaml_value_to_string(value);
                }
            }
        }

        // Return key as fallback
        key.to_string()
    }

    /// Get a pluralized translation (_p function)
    pub fn get_plural(&self, key: &str, count: usize) -> String {
        let plural_key = if count == 0 {
            format!("{}.zero", key)
        } else if count == 1 {
            format!("{}.one", key)
        } else {
            format!("{}.other", key)
        };

        let translation = self.get(&plural_key);

        // Replace %d with the count
        translation.replace("%d", &count.to_string())
    }

    /// Check if a translation exists
    pub fn has(&self, key: &str) -> bool {
        if let Some(lang_data) = self.translations.get(&self.language) {
            return get_nested_value(lang_data, key).is_some();
        }
        false
    }

    /// Get all translations for the current language as a flat HashMap
    /// This flattens nested keys using dot notation (e.g., "menu.home")
    pub fn get_all_translations(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();

        if let Some(lang_data) = self.translations.get(&self.language) {
            flatten_translations(lang_data, "", &mut result);
        }

        // Merge with English fallback for missing keys
        if self.language != "en" {
            if let Some(en_data) = self.translations.get("en") {
                let mut en_result = HashMap::new();
                flatten_translations(en_data, "", &mut en_result);
                for (k, v) in en_result {
                    result.entry(k).or_insert(v);
                }
            }
        }

        result
    }
}

/// Get a nested value from a YAML map using dot notation
fn get_nested_value<'a>(
    data: &'a HashMap<String, serde_yaml::Value>,
    key: &str,
) -> Option<&'a serde_yaml::Value> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut current: Option<&serde_yaml::Value> = data.get(parts[0]);

    for part in &parts[1..] {
        match current {
            Some(serde_yaml::Value::Mapping(map)) => {
                current = map.get(serde_yaml::Value::String(part.to_string()));
            }
            _ => return None,
        }
    }

    current
}

/// Convert a YAML value to a string
fn yaml_value_to_string(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        _ => format!("{:?}", value),
    }
}

/// Flatten translations into a HashMap with dot-notation keys
fn flatten_translations(
    data: &HashMap<String, serde_yaml::Value>,
    prefix: &str,
    result: &mut HashMap<String, String>,
) {
    for (key, value) in data {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        match value {
            serde_yaml::Value::String(s) => {
                result.insert(full_key, s.clone());
            }
            serde_yaml::Value::Number(n) => {
                result.insert(full_key, n.to_string());
            }
            serde_yaml::Value::Bool(b) => {
                result.insert(full_key, b.to_string());
            }
            serde_yaml::Value::Mapping(map) => {
                // Recursively flatten nested objects
                let mut nested = HashMap::new();
                for (k, v) in map {
                    if let serde_yaml::Value::String(key_str) = k {
                        nested.insert(key_str.clone(), v.clone());
                    }
                }
                flatten_translations(&nested, &full_key, result);
            }
            _ => {}
        }
    }
}

/// Convert JSON value to YAML HashMap
fn convert_json_to_yaml(json: serde_json::Value) -> HashMap<String, serde_yaml::Value> {
    let mut result = HashMap::new();

    if let serde_json::Value::Object(obj) = json {
        for (key, value) in obj {
            result.insert(key, json_value_to_yaml(value));
        }
    }

    result
}

fn json_value_to_yaml(json: serde_json::Value) -> serde_yaml::Value {
    match json {
        serde_json::Value::Null => serde_yaml::Value::Null,
        serde_json::Value::Bool(b) => serde_yaml::Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_yaml::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::Null
            }
        }
        serde_json::Value::String(s) => serde_yaml::Value::String(s),
        serde_json::Value::Array(arr) => {
            serde_yaml::Value::Sequence(arr.into_iter().map(json_value_to_yaml).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = serde_yaml::Mapping::new();
            for (k, v) in obj {
                map.insert(serde_yaml::Value::String(k), json_value_to_yaml(v));
            }
            serde_yaml::Value::Mapping(map)
        }
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new("en")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_translation() {
        let mut i18n = I18n::new("en");
        let mut en_data = HashMap::new();
        en_data.insert(
            "hello".to_string(),
            serde_yaml::Value::String("Hello".to_string()),
        );

        let mut menu = serde_yaml::Mapping::new();
        menu.insert(
            serde_yaml::Value::String("home".to_string()),
            serde_yaml::Value::String("Home".to_string()),
        );
        en_data.insert("menu".to_string(), serde_yaml::Value::Mapping(menu));

        i18n.translations.insert("en".to_string(), en_data);

        assert_eq!(i18n.get("hello"), "Hello");
        assert_eq!(i18n.get("menu.home"), "Home");
        assert_eq!(i18n.get("unknown"), "unknown");
    }

    #[test]
    fn test_get_all_translations() {
        let mut i18n = I18n::new("en");
        let mut en_data = HashMap::new();
        en_data.insert(
            "powered_by".to_string(),
            serde_yaml::Value::String("Powered by".to_string()),
        );
        en_data.insert(
            "home".to_string(),
            serde_yaml::Value::String("Home".to_string()),
        );

        let mut menu = serde_yaml::Mapping::new();
        menu.insert(
            serde_yaml::Value::String("archives".to_string()),
            serde_yaml::Value::String("Archives".to_string()),
        );
        en_data.insert("menu".to_string(), serde_yaml::Value::Mapping(menu));

        i18n.translations.insert("en".to_string(), en_data);

        let all = i18n.get_all_translations();
        assert_eq!(all.get("powered_by"), Some(&"Powered by".to_string()));
        assert_eq!(all.get("home"), Some(&"Home".to_string()));
        assert_eq!(all.get("menu.archives"), Some(&"Archives".to_string()));
    }
}
