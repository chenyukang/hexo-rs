//! URL helper functions

use crate::config::SiteConfig;

/// Generate a URL with the root path
///
/// # Examples
/// ```ignore
/// url_for(&config, "/css/style.css") // -> "/blog/css/style.css"
/// ```
pub fn url_for(config: &SiteConfig, path: &str) -> String {
    let root = config.root.trim_end_matches('/');
    let path = path.trim_start_matches('/');

    if path.is_empty() {
        format!("{}/", root)
    } else {
        format!("{}/{}", root, path)
    }
}

/// Generate a full URL including the domain
///
/// # Examples
/// ```ignore
/// full_url_for(&config, "/about/") // -> "https://example.com/blog/about/"
/// ```
pub fn full_url_for(config: &SiteConfig, path: &str) -> String {
    let base = config.url.trim_end_matches('/');
    let path = url_for(config, path);

    // Avoid double slashes
    if path.starts_with('/') && base.ends_with('/') {
        format!("{}{}", base.trim_end_matches('/'), path)
    } else {
        format!("{}{}", base, path)
    }
}

/// Calculate relative URL from one path to another
///
/// # Examples
/// ```ignore
/// relative_url("/foo/bar/", "/css/style.css") // -> "../../css/style.css"
/// ```
pub fn relative_url(from: &str, to: &str) -> String {
    let from_parts: Vec<&str> = from.trim_matches('/').split('/').collect();
    let to_parts: Vec<&str> = to.trim_matches('/').split('/').collect();

    // Find common prefix
    let mut common = 0;
    for (i, part) in from_parts.iter().enumerate() {
        if i < to_parts.len() && *part == to_parts[i] {
            common = i + 1;
        } else {
            break;
        }
    }

    // Build relative path
    let up_count = from_parts.len() - common;
    let mut result = String::new();

    for _ in 0..up_count {
        result.push_str("../");
    }

    for part in &to_parts[common..] {
        result.push_str(part);
        result.push('/');
    }

    if result.is_empty() {
        "./".to_string()
    } else {
        result.trim_end_matches('/').to_string()
    }
}

/// Encode a URL path
pub fn encode_url(path: &str) -> String {
    percent_encoding::utf8_percent_encode(path, percent_encoding::NON_ALPHANUMERIC).to_string()
}

/// Gravatar URL helper
pub fn gravatar(email: &str, size: Option<u32>) -> String {
    let hash = md5_hash(email.trim().to_lowercase().as_bytes());
    let size = size.unwrap_or(80);
    format!("https://www.gravatar.com/avatar/{}?s={}", hash, size)
}

/// Simple MD5 hash (for gravatar)
fn md5_hash(data: &[u8]) -> String {
    // Simple implementation - in production, use a proper md5 crate
    // For now, return a placeholder
    let mut hash = String::with_capacity(32);
    for byte in data.iter().take(16) {
        hash.push_str(&format!("{:02x}", byte));
    }
    while hash.len() < 32 {
        hash.push('0');
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SiteConfig {
        let mut config = SiteConfig::default();
        config.url = "https://example.com".to_string();
        config.root = "/blog/".to_string();
        config
    }

    #[test]
    fn test_url_for() {
        let config = test_config();
        assert_eq!(url_for(&config, "/css/style.css"), "/blog/css/style.css");
        assert_eq!(url_for(&config, "about/"), "/blog/about/");
    }

    #[test]
    fn test_full_url_for() {
        let config = test_config();
        assert_eq!(
            full_url_for(&config, "/about/"),
            "https://example.com/blog/about/"
        );
    }

    #[test]
    fn test_relative_url() {
        assert_eq!(
            relative_url("/foo/bar/", "/css/style.css"),
            "../../css/style.css"
        );
    }
}
