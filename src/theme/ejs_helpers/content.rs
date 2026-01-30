//! Content-related helpers: toc, truncate, paginator

/// Generate table of contents from HTML content with custom class
pub fn toc(content: &str, max_depth: usize, class_name: &str) -> String {
    let toc_html = crate::helpers::toc(content, max_depth);

    // If no headings found, return empty
    if !toc_html.contains("<li") {
        return String::new();
    }

    // Replace default class names with custom class name
    toc_html
        .replace("class=\"toc-item", &format!("class=\"{}-item", class_name))
        .replace("class=\"toc-link", &format!("class=\"{}-link", class_name))
        .replace("class=\"toc-text", &format!("class=\"{}-text", class_name))
        .replace("toc-level-", &format!("{}-level-", class_name))
        .replace("<ol>", &format!("<ol class=\"{}-child\">", class_name))
        .replace("class=\"toc\"", &format!("class=\"{}\"", class_name))
}

/// Truncate text to a specified length (does NOT strip HTML first)
pub fn truncate(content: &str, length: usize, end: &str) -> String {
    if content.chars().count() <= length {
        content.to_string()
    } else {
        let truncated: String = content.chars().take(length).collect();
        format!("{}{}", truncated.trim_end(), end)
    }
}

/// Strip HTML tags from content
pub fn strip_html(content: &str) -> String {
    strip_html_tags(content)
}

/// Generate paginator HTML
pub fn paginator(
    current: usize,
    total: usize,
    root: &str,
    base_path: &str,
    prev_text: &str,
    next_text: &str,
    mid_size: usize,
) -> String {
    if total <= 1 {
        return String::new();
    }

    let root = root.trim_end_matches('/');
    let mut html = String::from(r#"<nav class="pagination">"#);

    // Previous button
    if current > 1 {
        let prev_url = if current == 2 {
            format!("{}/{}", root, base_path.trim_start_matches('/'))
        } else {
            format!(
                "{}/{}page/{}/",
                root,
                base_path.trim_start_matches('/'),
                current - 1
            )
        };
        html.push_str(&format!(
            r#"<a class="pagination-prev" href="{}">{}</a>"#,
            prev_url, prev_text
        ));
    }

    // Page numbers
    let start = if current > mid_size {
        current - mid_size
    } else {
        1
    };
    let end = if current + mid_size <= total {
        current + mid_size
    } else {
        total
    };

    for i in start..=end {
        let page_url = if i == 1 {
            format!("{}/{}", root, base_path.trim_start_matches('/'))
        } else {
            format!("{}/{}page/{}/", root, base_path.trim_start_matches('/'), i)
        };

        if i == current {
            html.push_str(&format!(
                r#"<span class="pagination-number current">{}</span>"#,
                i
            ));
        } else {
            html.push_str(&format!(
                r#"<a class="pagination-number" href="{}">{}</a>"#,
                page_url, i
            ));
        }
    }

    // Next button
    if current < total {
        let next_url = format!(
            "{}/{}page/{}/",
            root,
            base_path.trim_start_matches('/'),
            current + 1
        );
        html.push_str(&format!(
            r#"<a class="pagination-next" href="{}">{}</a>"#,
            next_url, next_text
        ));
    }

    html.push_str("</nav>");
    html
}

/// Strip HTML tags from content
fn strip_html_tags(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("Hello World", 5, "..."), "Hello...");
        assert_eq!(truncate("Hi", 10, "..."), "Hi");
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
        assert_eq!(strip_html_tags("<a href='#'>Link</a>"), "Link");
    }
}
