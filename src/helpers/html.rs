//! HTML helper functions

use super::url::url_for;
use crate::config::SiteConfig;

/// Generate a CSS link tag
///
/// # Examples
/// ```ignore
/// css(&config, "style.css") // -> <link rel="stylesheet" href="/blog/css/style.css">
/// ```
pub fn css(config: &SiteConfig, path: &str) -> String {
    let path =
        if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("//") {
            path.to_string()
        } else {
            let path = if path.ends_with(".css") {
                path.to_string()
            } else {
                format!("{}.css", path)
            };
            url_for(config, &format!("css/{}", path.trim_start_matches('/')))
        };

    format!(r#"<link rel="stylesheet" href="{}">"#, path)
}

/// Generate a JavaScript script tag
///
/// # Examples
/// ```ignore
/// js(&config, "app.js") // -> <script src="/blog/js/app.js"></script>
/// ```
pub fn js(config: &SiteConfig, path: &str) -> String {
    let path =
        if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("//") {
            path.to_string()
        } else {
            let path = if path.ends_with(".js") {
                path.to_string()
            } else {
                format!("{}.js", path)
            };
            url_for(config, &format!("js/{}", path.trim_start_matches('/')))
        };

    format!(r#"<script src="{}"></script>"#, path)
}

/// Generate an anchor tag
///
/// # Examples
/// ```ignore
/// link_to(&config, "/about/", "About", false) // -> <a href="/blog/about/">About</a>
/// ```
pub fn link_to(config: &SiteConfig, path: &str, text: &str, external: bool) -> String {
    let href = if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        url_for(config, path)
    };

    if external || path.starts_with("http://") || path.starts_with("https://") {
        format!(
            r#"<a href="{}" target="_blank" rel="noopener">{}</a>"#,
            href, text
        )
    } else {
        format!(r#"<a href="{}">{}</a>"#, href, text)
    }
}

/// Generate an image tag
///
/// # Examples
/// ```ignore
/// image_tag(&config, "/images/photo.jpg", Some("My Photo"), None)
/// ```
pub fn image_tag(
    config: &SiteConfig,
    path: &str,
    alt: Option<&str>,
    title: Option<&str>,
) -> String {
    let src = if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        url_for(config, path)
    };

    let alt = alt.unwrap_or("");
    let title_attr = title
        .map(|t| format!(r#" title="{}""#, html_escape(t)))
        .unwrap_or_default();

    format!(
        r#"<img src="{}" alt="{}"{}>"#,
        src,
        html_escape(alt),
        title_attr
    )
}

/// Generate a favicon link tag
pub fn favicon_tag(config: &SiteConfig, path: &str) -> String {
    let href = url_for(config, path);
    format!(r#"<link rel="icon" href="{}">"#, href)
}

/// Generate a feed/RSS link tag
pub fn feed_tag(config: &SiteConfig, path: &str, title: Option<&str>) -> String {
    let href = url_for(config, path);
    let title = title.unwrap_or(&config.title);
    format!(
        r#"<link rel="alternate" href="{}" title="{}" type="application/atom+xml">"#,
        href,
        html_escape(title)
    )
}

/// Generate Open Graph meta tags
pub fn open_graph(
    title: &str,
    description: &str,
    url: &str,
    image: Option<&str>,
    site_name: &str,
) -> String {
    let mut tags = vec![
        format!(r#"<meta property="og:type" content="website">"#),
        format!(
            r#"<meta property="og:title" content="{}">"#,
            html_escape(title)
        ),
        format!(r#"<meta property="og:url" content="{}">"#, url),
        format!(
            r#"<meta property="og:site_name" content="{}">"#,
            html_escape(site_name)
        ),
    ];

    if !description.is_empty() {
        tags.push(format!(
            r#"<meta property="og:description" content="{}">"#,
            html_escape(description)
        ));
    }

    if let Some(img) = image {
        tags.push(format!(r#"<meta property="og:image" content="{}">"#, img));
    }

    tags.join("\n")
}

/// Generate meta generator tag
pub fn meta_generator() -> String {
    format!(
        r#"<meta name="generator" content="hexo-rs {}">"#,
        env!("CARGO_PKG_VERSION")
    )
}

/// Escape HTML special characters
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Strip HTML tags from a string
pub fn strip_html(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;

    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    result
}

/// Truncate a string to a specified length
pub fn truncate(s: &str, length: usize, omission: Option<&str>) -> String {
    let omission = omission.unwrap_or("...");

    if s.chars().count() <= length {
        s.to_string()
    } else {
        let truncated: String = s
            .chars()
            .take(length.saturating_sub(omission.len()))
            .collect();
        format!("{}{}", truncated.trim_end(), omission)
    }
}

/// Word wrap text
pub fn word_wrap(s: &str, width: usize) -> String {
    let mut result = String::new();
    let mut line_len = 0;

    for word in s.split_whitespace() {
        let word_len = word.len();
        if line_len + word_len + 1 > width {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(word);
            line_len = word_len;
        } else {
            if !result.is_empty() {
                result.push(' ');
                line_len += 1;
            }
            result.push_str(word);
            line_len += word_len;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SiteConfig {
        let mut config = SiteConfig::default();
        config.root = "/".to_string();
        config
    }

    #[test]
    fn test_css() {
        let config = test_config();
        assert!(css(&config, "style").contains("style.css"));
        assert!(css(&config, "style").contains("<link"));
    }

    #[test]
    fn test_js() {
        let config = test_config();
        assert!(js(&config, "app").contains("app.js"));
        assert!(js(&config, "app").contains("<script"));
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html("<p>Hello <b>World</b></p>"), "Hello World");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello World", 8, None), "Hello...");
        assert_eq!(truncate("Hi", 10, None), "Hi");
    }
}
