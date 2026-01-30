//! URL-related helper functions
//!
//! These functions generate URLs and HTML tags for assets.

use crate::theme::ejs::EjsValue;

/// Generate a URL with root prefix
pub fn url_for(root: &str, path: Option<&str>) -> String {
    let root = root.trim_end_matches('/');
    match path {
        Some(p) => {
            let p = p.trim_start_matches('/');
            if p.is_empty() {
                format!("{}/", root)
            } else {
                format!("{}/{}", root, p)
            }
        }
        None => format!("{}/", root),
    }
}

/// Generate CSS link tag(s)
pub fn css(root: &str, value: &EjsValue) -> String {
    let root = root.trim_end_matches('/');

    let generate_link = |path: String| -> String {
        let href = if path.starts_with("http://")
            || path.starts_with("https://")
            || path.starts_with("//")
        {
            path
        } else {
            let path = if path.ends_with(".css") {
                path
            } else {
                format!("{}.css", path)
            };
            format!("{}/{}", root, path.trim_start_matches('/'))
        };
        format!(r#"<link rel="stylesheet" href="{}">"#, href)
    };

    match value {
        EjsValue::Array(items) => items
            .iter()
            .map(|item| generate_link(item.to_output_string()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => generate_link(value.to_output_string()),
    }
}

/// Generate JS script tag(s)
pub fn js(root: &str, value: &EjsValue) -> String {
    let root = root.trim_end_matches('/');

    let generate_script = |path: String| -> String {
        let src = if path.starts_with("http://")
            || path.starts_with("https://")
            || path.starts_with("//")
        {
            path
        } else {
            let path = if path.ends_with(".js") {
                path
            } else {
                format!("{}.js", path)
            };
            format!("{}/{}", root, path.trim_start_matches('/'))
        };
        format!(r#"<script src="{}"></script>"#, src)
    };

    match value {
        EjsValue::Array(items) => items
            .iter()
            .map(|item| generate_script(item.to_output_string()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => generate_script(value.to_output_string()),
    }
}

/// Generate favicon link tag
/// Returns None if path is empty (allows caller to skip output)
pub fn favicon_tag(root: &str, path: &str) -> String {
    let root = root.trim_end_matches('/');

    // Handle external URLs
    let href =
        if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("//") {
            path.to_string()
        } else {
            let normalized = if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/{}", path)
            };
            format!("{}{}", root, normalized)
        };

    format!(r#"<link rel="icon" href="{}">"#, href)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_for() {
        assert_eq!(url_for("/", Some("about")), "/about");
        assert_eq!(url_for("/blog", Some("about")), "/blog/about");
        assert_eq!(url_for("/", Some("/about/")), "/about/");
        assert_eq!(url_for("/", None), "/");
    }

    #[test]
    fn test_css() {
        let result = css("/", &EjsValue::String("style".to_string()));
        assert!(result.contains("href=\"/style.css\""));

        let result = css(
            "/",
            &EjsValue::String("https://cdn.example.com/style.css".to_string()),
        );
        assert!(result.contains("href=\"https://cdn.example.com/style.css\""));
    }

    #[test]
    fn test_js() {
        let result = js("/", &EjsValue::String("app".to_string()));
        assert!(result.contains("src=\"/app.js\""));
    }
}
