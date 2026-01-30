//! List-related helpers: list_categories, list_tags, list_archives, tagcloud

use std::collections::HashMap;

/// Simple percent encoding for URL paths
fn percent_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    result
}

/// Generate category list HTML
pub fn list_categories(
    categories: &HashMap<String, usize>,
    root: &str,
    show_count: bool,
    class: &str,
) -> String {
    let root = root.trim_end_matches('/');

    if categories.is_empty() {
        return String::new();
    }

    let mut sorted: Vec<_> = categories.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut html = format!(r#"<ul class="{}">"#, class);

    for (name, count) in sorted {
        let url = format!("{}/categories/{}/", root, name);
        html.push_str(&format!(r#"<li class="{}-list-item">"#, class));
        html.push_str(&format!(
            r#"<a class="{}-list-link" href="{}">{}</a>"#,
            class, url, name
        ));
        if show_count {
            html.push_str(&format!(
                r#"<span class="{}-list-count">{}</span>"#,
                class, count
            ));
        }
        html.push_str("</li>");
    }

    html.push_str("</ul>");
    html
}

/// Generate tag list HTML
pub fn list_tags(
    tags: &HashMap<String, usize>,
    root: &str,
    show_count: bool,
    class: &str,
) -> String {
    let root = root.trim_end_matches('/');

    if tags.is_empty() {
        return String::new();
    }

    let mut sorted: Vec<_> = tags.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut html = format!(r#"<ul class="{}">"#, class);

    for (name, count) in sorted {
        let url = format!("{}/tags/{}/", root, percent_encode(name));
        html.push_str(&format!(r#"<li class="{}-list-item">"#, class));
        html.push_str(&format!(
            r#"<a class="{}-list-link" href="{}">{}</a>"#,
            class, url, name
        ));
        if show_count {
            html.push_str(&format!(
                r#"<span class="{}-list-count">{}</span>"#,
                class, count
            ));
        }
        html.push_str("</li>");
    }

    html.push_str("</ul>");
    html
}

/// Archive entry for list_archives
pub struct ArchiveEntry {
    pub year: i32,
    pub month: Option<u32>,
    pub count: usize,
}

/// Generate archive list HTML
pub fn list_archives(
    archives: &[ArchiveEntry],
    root: &str,
    show_count: bool,
    _format: &str, // "YYYY" for year only, "YYYY-MM" for year-month (reserved for future)
) -> String {
    let root = root.trim_end_matches('/');

    if archives.is_empty() {
        return String::new();
    }

    let mut html = String::from(r#"<ul class="archive-list">"#);

    for entry in archives {
        let (url, display) = if let Some(month) = entry.month {
            (
                format!("{}/archives/{}/{:02}/", root, entry.year, month),
                format!("{}-{:02}", entry.year, month),
            )
        } else {
            (
                format!("{}/archives/{}/", root, entry.year),
                format!("{}", entry.year),
            )
        };

        html.push_str(r#"<li class="archive-list-item">"#);
        html.push_str(&format!(
            r#"<a class="archive-list-link" href="{}">{}</a>"#,
            url, display
        ));
        if show_count {
            html.push_str(&format!(
                r#"<span class="archive-list-count">{}</span>"#,
                entry.count
            ));
        }
        html.push_str("</li>");
    }

    html.push_str("</ul>");
    html
}

/// Generate tag cloud HTML
pub fn tagcloud(
    tags: &HashMap<String, usize>,
    root: &str,
    min_font: f64,
    max_font: f64,
    unit: &str,
) -> String {
    let root = root.trim_end_matches('/');

    if tags.is_empty() {
        return String::new();
    }

    let min_count = *tags.values().min().unwrap_or(&1) as f64;
    let max_count = *tags.values().max().unwrap_or(&1) as f64;
    let range = if max_count > min_count {
        max_count - min_count
    } else {
        1.0
    };

    let mut sorted: Vec<_> = tags.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut html = String::new();

    for (name, count) in sorted {
        let size = if range > 0.0 {
            min_font + (*count as f64 - min_count) / range * (max_font - min_font)
        } else {
            min_font
        };
        let url = format!("{}/tags/{}/", root, percent_encode(name));
        html.push_str(&format!(
            r#"<a href="{}" style="font-size: {:.2}{}">{}</a> "#,
            url, size, unit, name
        ));
    }

    html.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_categories() {
        let mut cats = HashMap::new();
        cats.insert("Rust".to_string(), 5);
        cats.insert("Go".to_string(), 3);

        let result = list_categories(&cats, "/", true, "category");
        assert!(result.contains("Rust"));
        assert!(result.contains("Go"));
        assert!(result.contains("category-list-count"));
    }

    #[test]
    fn test_tagcloud() {
        let mut tags = HashMap::new();
        tags.insert("rust".to_string(), 10);
        tags.insert("go".to_string(), 5);

        let result = tagcloud(&tags, "/", 10.0, 20.0, "px");
        assert!(result.contains("rust"));
        assert!(result.contains("font-size"));
    }
}
