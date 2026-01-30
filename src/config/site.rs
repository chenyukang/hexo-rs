//! Site configuration (_config.yml)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Main site configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteConfig {
    // Site
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub keywords: Option<Vec<String>>,
    pub author: String,
    pub language: String,
    pub timezone: String,

    // URL
    pub url: String,
    pub root: String,
    pub permalink: String,
    #[serde(default)]
    pub permalink_defaults: HashMap<String, String>,
    #[serde(default)]
    pub pretty_urls: PrettyUrlsConfig,

    // Directory
    pub source_dir: String,
    pub public_dir: String,
    pub tag_dir: String,
    pub archive_dir: String,
    pub category_dir: String,
    pub code_dir: String,
    pub i18n_dir: String,
    #[serde(default)]
    pub skip_render: Vec<String>,

    // Writing
    pub new_post_name: String,
    pub default_layout: String,
    pub titlecase: bool,
    #[serde(default)]
    pub external_link: ExternalLinkConfig,
    pub filename_case: i32,
    pub render_drafts: bool,
    pub post_asset_folder: bool,
    pub relative_link: bool,
    pub future: bool,
    pub syntax_highlighter: String,
    #[serde(default)]
    pub highlight: HighlightConfig,
    #[serde(default)]
    pub prismjs: PrismjsConfig,

    // Home page
    #[serde(default)]
    pub index_generator: IndexGeneratorConfig,

    // Category & Tag
    pub default_category: String,
    #[serde(default)]
    pub category_map: HashMap<String, String>,
    #[serde(default)]
    pub tag_map: HashMap<String, String>,

    // Meta
    pub meta_generator: bool,

    // Date / Time format
    pub date_format: String,
    pub time_format: String,
    pub updated_option: String,

    // Pagination
    pub per_page: usize,
    pub pagination_dir: String,

    // Extensions
    pub theme: String,
    #[serde(default)]
    pub theme_config: HashMap<String, serde_yaml::Value>,

    // Store any additional fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "Hexo".to_string(),
            subtitle: String::new(),
            description: String::new(),
            keywords: None,
            author: "John Doe".to_string(),
            language: "en".to_string(),
            timezone: String::new(),

            url: "http://example.com".to_string(),
            root: "/".to_string(),
            permalink: ":year/:month/:day/:title/".to_string(),
            permalink_defaults: HashMap::new(),
            pretty_urls: PrettyUrlsConfig::default(),

            source_dir: "source".to_string(),
            public_dir: "public".to_string(),
            tag_dir: "tags".to_string(),
            archive_dir: "archives".to_string(),
            category_dir: "categories".to_string(),
            code_dir: "downloads/code".to_string(),
            i18n_dir: ":lang".to_string(),
            skip_render: Vec::new(),

            new_post_name: ":title.md".to_string(),
            default_layout: "post".to_string(),
            titlecase: false,
            external_link: ExternalLinkConfig::default(),
            filename_case: 0,
            render_drafts: false,
            post_asset_folder: false,
            relative_link: false,
            future: true,
            syntax_highlighter: "highlight.js".to_string(),
            highlight: HighlightConfig::default(),
            prismjs: PrismjsConfig::default(),

            index_generator: IndexGeneratorConfig::default(),

            default_category: "uncategorized".to_string(),
            category_map: HashMap::new(),
            tag_map: HashMap::new(),

            meta_generator: true,

            date_format: "YYYY-MM-DD".to_string(),
            time_format: "HH:mm:ss".to_string(),
            updated_option: "mtime".to_string(),

            per_page: 10,
            pagination_dir: "page".to_string(),

            theme: "landscape".to_string(),
            theme_config: HashMap::new(),
            extra: HashMap::new(),
        }
    }
}

impl SiteConfig {
    /// Load configuration from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())?;
        let config: SiteConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Merge with theme configuration
    pub fn merge_theme_config(&mut self, theme_config: HashMap<String, serde_yaml::Value>) {
        for (key, value) in theme_config {
            self.theme_config.insert(key, value);
        }
    }

    /// Load theme-specific config from _config.[theme].yml
    pub fn load_theme_override<P: AsRef<Path>>(&mut self, base_dir: P) -> Result<()> {
        let theme_config_path = base_dir
            .as_ref()
            .join(format!("_config.{}.yml", self.theme));

        if theme_config_path.exists() {
            let content = fs::read_to_string(&theme_config_path)?;
            let theme_config: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(&content)?;
            self.merge_theme_config(theme_config);
            tracing::debug!("Loaded theme override from {:?}", theme_config_path);
        }

        Ok(())
    }
}

/// Pretty URL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrettyUrlsConfig {
    pub trailing_index: bool,
    pub trailing_html: bool,
}

impl Default for PrettyUrlsConfig {
    fn default() -> Self {
        Self {
            trailing_index: true,
            trailing_html: true,
        }
    }
}

/// External link configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalLinkConfig {
    pub enable: bool,
    pub field: String,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for ExternalLinkConfig {
    fn default() -> Self {
        Self {
            enable: true,
            field: "site".to_string(),
            exclude: Vec::new(),
        }
    }
}

/// Highlight.js configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightConfig {
    pub auto_detect: bool,
    pub line_number: bool,
    pub line_threshold: usize,
    pub tab_replace: String,
    #[serde(default)]
    pub exclude_languages: Vec<String>,
    pub wrap: bool,
    pub hljs: bool,
}

impl Default for HighlightConfig {
    fn default() -> Self {
        Self {
            auto_detect: false,
            line_number: true,
            line_threshold: 0,
            tab_replace: String::new(),
            exclude_languages: Vec::new(),
            wrap: true,
            hljs: false,
        }
    }
}

/// PrismJS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrismjsConfig {
    pub preprocess: bool,
    pub line_number: bool,
    pub line_threshold: usize,
    pub tab_replace: String,
}

impl Default for PrismjsConfig {
    fn default() -> Self {
        Self {
            preprocess: true,
            line_number: true,
            line_threshold: 0,
            tab_replace: String::new(),
        }
    }
}

/// Index generator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexGeneratorConfig {
    pub path: String,
    pub per_page: usize,
    pub order_by: String,
    pub pagination_dir: String,
}

impl Default for IndexGeneratorConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            per_page: 10,
            order_by: "-date".to_string(),
            pagination_dir: "page".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SiteConfig::default();
        assert_eq!(config.title, "Hexo");
        assert_eq!(config.theme, "landscape");
        assert_eq!(config.per_page, 10);
    }

    #[test]
    fn test_parse_config() {
        let yaml = r#"
title: My Blog
author: Test User
theme: next
per_page: 20
"#;
        let config: SiteConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.title, "My Blog");
        assert_eq!(config.author, "Test User");
        assert_eq!(config.theme, "next");
        assert_eq!(config.per_page, 20);
    }
}
