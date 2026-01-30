//! Front-matter parsing

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, NaiveDateTime};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Custom deserializer that handles both a single string and a list of strings
fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    struct StringOrVec;

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value])
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(item) = seq.next_element::<String>()? {
                vec.push(item);
            }
            Ok(vec)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Vec::new())
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

/// Front-matter data from a post or page
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FrontMatter {
    pub title: Option<String>,
    pub date: Option<String>,
    pub updated: Option<String>,
    pub comments: bool,
    #[serde(deserialize_with = "string_or_vec", default)]
    pub tags: Vec<String>,
    #[serde(deserialize_with = "string_or_vec", default)]
    pub categories: Vec<String>,
    pub layout: Option<String>,
    pub permalink: Option<String>,
    pub excerpt: Option<String>,
    /// Posts are published by default (Hexo behavior)
    #[serde(default = "default_published")]
    pub published: bool,
    pub lang: Option<String>,
    #[serde(rename = "disableNunjucks")]
    pub disable_nunjucks: bool,

    /// Additional custom fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// Default value for published field - true to match Hexo behavior
fn default_published() -> bool {
    true
}

impl Default for FrontMatter {
    fn default() -> Self {
        Self {
            title: None,
            date: None,
            updated: None,
            comments: false,
            tags: Vec::new(),
            categories: Vec::new(),
            layout: None,
            permalink: None,
            excerpt: None,
            published: true, // Posts are published by default
            lang: None,
            disable_nunjucks: false,
            extra: HashMap::new(),
        }
    }
}

impl FrontMatter {
    /// Parse front-matter from content string
    /// Returns (front_matter, remaining_content)
    pub fn parse(content: &str) -> Result<(Self, &str)> {
        let content = content.trim_start();

        // Check for YAML front-matter (---)
        if content.starts_with("---") {
            return Self::parse_yaml(content);
        }

        // Check for JSON front-matter (;;; or {"key":)
        if content.starts_with(";;;") || content.starts_with('{') {
            return Self::parse_json(content);
        }

        // No front-matter found
        Ok((FrontMatter::default(), content))
    }

    fn parse_yaml(content: &str) -> Result<(Self, &str)> {
        // Find the closing ---
        let rest = &content[3..]; // Skip opening ---
        let rest = rest.trim_start_matches(['\n', '\r']);

        if let Some(end_pos) = rest.find("\n---") {
            let yaml_content = &rest[..end_pos];
            let remaining = &rest[end_pos + 4..]; // Skip \n---
            let remaining = remaining.trim_start_matches(['\n', '\r']);

            // If YAML content is empty or whitespace-only, return default
            if yaml_content.trim().is_empty() {
                return Ok((FrontMatter::default(), remaining));
            }

            // Check if this looks like valid YAML (should have key: value format)
            // Valid YAML front-matter should have at least one line with 'key: value' pattern
            // Skip content that looks like prose or markdown
            let has_yaml_structure = yaml_content.lines().any(|line| {
                let trimmed = line.trim();
                // Skip empty lines and comments
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return false;
                }
                // Look for "key:" pattern - colon should be followed by space, newline, or end
                // This is the primary indicator of YAML front-matter
                // Note: We don't check for "- item" alone because that's also valid markdown list syntax
                if let Some(colon_pos) = trimmed.find(':') {
                    let before_colon = &trimmed[..colon_pos];
                    // Key should be a simple ASCII identifier (letters, numbers, underscore, hyphen)
                    // and the colon should not be part of a URL (http:, https:, etc.)
                    let is_valid_key = !before_colon.is_empty()
                        && before_colon
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                        && before_colon != "http"
                        && before_colon != "https"
                        && before_colon != "ftp";
                    if is_valid_key {
                        // Check what comes after the colon
                        let after_colon = &trimmed[colon_pos + 1..];
                        // Should be empty, start with space, or be a valid YAML value
                        return after_colon.is_empty() || after_colon.starts_with(' ');
                    }
                }
                false
            });

            if !has_yaml_structure {
                // This doesn't look like YAML, treat as no front-matter
                // Return the original content
                return Ok((FrontMatter::default(), content));
            }

            match serde_yaml::from_str::<FrontMatter>(yaml_content) {
                Ok(fm) => Ok((fm, remaining)),
                Err(e) => {
                    // If YAML parsing fails, log a warning and return default
                    tracing::warn!(
                        "Failed to parse YAML front-matter, treating as content: {}",
                        e
                    );
                    Ok((FrontMatter::default(), content))
                }
            }
        } else {
            // No closing ---, treat as no front-matter
            Ok((FrontMatter::default(), content))
        }
    }

    fn parse_json(content: &str) -> Result<(Self, &str)> {
        // JSON front-matter ends with ;;;
        if let Some(rest) = content.strip_prefix(";;;") {
            if let Some(end_pos) = rest.find(";;;") {
                let json_content = &rest[..end_pos];
                let remaining = &rest[end_pos + 3..];
                let remaining = remaining.trim_start_matches(['\n', '\r']);

                let fm: FrontMatter = serde_json::from_str(json_content)
                    .map_err(|e| anyhow!("Failed to parse JSON front-matter: {}", e))?;

                return Ok((fm, remaining));
            }
        }

        // Try parsing as a JSON object at the start
        if content.starts_with('{') {
            // Find matching closing brace
            let mut depth = 0;
            let mut end_pos = 0;
            for (i, c) in content.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end_pos = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if end_pos > 0 {
                let json_content = &content[..end_pos];
                let remaining = &content[end_pos..];
                let remaining = remaining.trim_start_matches(['\n', '\r']);

                let fm: FrontMatter = serde_json::from_str(json_content)
                    .map_err(|e| anyhow!("Failed to parse JSON front-matter: {}", e))?;

                return Ok((fm, remaining));
            }
        }

        Err(anyhow!("Invalid JSON front-matter"))
    }

    /// Parse the date string into a DateTime
    pub fn parse_date(&self) -> Option<DateTime<Local>> {
        self.date.as_ref().and_then(|s| parse_date_string(s))
    }

    /// Parse the updated date string into a DateTime
    pub fn parse_updated(&self) -> Option<DateTime<Local>> {
        self.updated.as_ref().and_then(|s| parse_date_string(s))
    }
}

/// Parse a date string in various formats
fn parse_date_string(s: &str) -> Option<DateTime<Local>> {
    let s = s.trim();

    // Try various formats
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M",
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
    ];

    for fmt in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(DateTime::from_naive_utc_and_offset(
                dt,
                *Local::now().offset(),
            ));
        }
        // Try parsing date only
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            let dt = d.and_hms_opt(0, 0, 0)?;
            return Some(DateTime::from_naive_utc_and_offset(
                dt,
                *Local::now().offset(),
            ));
        }
    }

    // Try RFC 3339 / ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_frontmatter() {
        let content = r#"---
title: Hello World
date: 2024-01-15 10:30:00
tags:
  - rust
  - hexo
categories:
  - programming
---

This is the content.
"#;

        let (fm, remaining) = FrontMatter::parse(content).unwrap();
        assert_eq!(fm.title, Some("Hello World".to_string()));
        assert_eq!(fm.tags, vec!["rust", "hexo"]);
        assert_eq!(fm.categories, vec!["programming"]);
        assert!(remaining.contains("This is the content."));
    }

    #[test]
    fn test_parse_json_frontmatter() {
        let content = r#"{"title": "Test Post", "tags": ["a", "b"]}

This is content.
"#;

        let (fm, remaining) = FrontMatter::parse(content).unwrap();
        assert_eq!(fm.title, Some("Test Post".to_string()));
        assert_eq!(fm.tags, vec!["a", "b"]);
        assert!(remaining.contains("This is content."));
    }

    #[test]
    fn test_parse_date() {
        let fm = FrontMatter {
            date: Some("2024-01-15 10:30:00".to_string()),
            ..Default::default()
        };

        let dt = fm.parse_date().unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2024-01-15");
    }

    #[test]
    fn test_parse_single_string_tags() {
        let content = r#"---
title: Single Tag Post
date: 2024-01-15
tags: Notes
categories: Blog
---

Content here.
"#;

        let (fm, _) = FrontMatter::parse(content).unwrap();
        assert_eq!(fm.title, Some("Single Tag Post".to_string()));
        assert_eq!(fm.tags, vec!["Notes"]);
        assert_eq!(fm.categories, vec!["Blog"]);
    }

    #[test]
    fn test_markdown_separator_not_yaml() {
        // Content that uses --- as markdown separator, not YAML front-matter
        let content = r#"
---

Some random text with markdown lists:
- Item 1
- Item 2

-- 2025-11-09

---
More content here.
"#;

        let (fm, remaining) = FrontMatter::parse(content).unwrap();
        // Should return default front-matter since there's no valid YAML structure
        assert_eq!(fm.title, None);
        // The content should be returned as-is (or starting from the original position)
        assert!(remaining.contains("Some random text"));
    }

    #[test]
    fn test_content_with_url_not_yaml() {
        // Content with URLs containing colons should not be mistaken for YAML
        let content = r#"
---

Check out https://example.com/path and http://test.com

---
More content.
"#;

        let (fm, remaining) = FrontMatter::parse(content).unwrap();
        assert_eq!(fm.title, None);
        assert!(remaining.contains("https://example.com"));
    }

    #[test]
    fn test_ideas_format_not_yaml() {
        // Ideas page format with --- separators should not be mistaken for YAML
        let content = r#"
---
最近用 AI 写代码的比例又提高了不少，几乎有一半的时间是在 vibe coding 了，软件开发的模式彻底变了。
-- 2026-01-26

---
第二条内容
-- 2025-12-31

---
"#;

        let (fm, remaining) = FrontMatter::parse(content).unwrap();
        // Should return default front-matter since there's no valid YAML structure
        assert_eq!(fm.title, None);
        // The content should contain the first idea
        assert!(remaining.contains("vibe coding"));
        assert!(remaining.contains("第二条内容"));
    }
}
