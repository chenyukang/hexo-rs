//! List helper functions for generating tag clouds, category lists, etc.

use std::collections::HashMap;

use super::url::url_for;
use crate::config::SiteConfig;
use crate::content::Post;

/// Generate a list of categories as HTML
pub fn list_categories(
    config: &SiteConfig,
    posts: &[Post],
    show_count: bool,
    class: Option<&str>,
) -> String {
    let mut categories: HashMap<String, usize> = HashMap::new();

    for post in posts {
        for cat in &post.categories {
            *categories.entry(cat.clone()).or_insert(0) += 1;
        }
    }

    if categories.is_empty() {
        return String::new();
    }

    let class = class.unwrap_or("category-list");
    let mut html = format!(r#"<ul class="{}">"#, class);

    let mut sorted: Vec<_> = categories.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    for (name, count) in sorted {
        let slug = slug::slugify(name);
        let url = url_for(config, &format!("{}/{}/", config.category_dir, slug));

        html.push_str(&format!(
            r#"<li class="{}-item"><a class="{}-link" href="{}">{}</a>"#,
            class, class, url, name
        ));

        if show_count {
            html.push_str(&format!(
                r#"<span class="{}-count">{}</span>"#,
                class, count
            ));
        }

        html.push_str("</li>");
    }

    html.push_str("</ul>");
    html
}

/// Generate a list of tags as HTML
pub fn list_tags(
    config: &SiteConfig,
    posts: &[Post],
    show_count: bool,
    class: Option<&str>,
) -> String {
    let mut tags: HashMap<String, usize> = HashMap::new();

    for post in posts {
        for tag in &post.tags {
            *tags.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    if tags.is_empty() {
        return String::new();
    }

    let class = class.unwrap_or("tag-list");
    let mut html = format!(r#"<ul class="{}">"#, class);

    let mut sorted: Vec<_> = tags.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    for (name, count) in sorted {
        let slug = slug::slugify(name);
        let url = url_for(config, &format!("{}/{}/", config.tag_dir, slug));

        html.push_str(&format!(
            r#"<li class="{}-item"><a class="{}-link" href="{}">{}</a>"#,
            class, class, url, name
        ));

        if show_count {
            html.push_str(&format!(
                r#"<span class="{}-count">{}</span>"#,
                class, count
            ));
        }

        html.push_str("</li>");
    }

    html.push_str("</ul>");
    html
}

/// Generate a list of archives as HTML
pub fn list_archives(
    config: &SiteConfig,
    posts: &[Post],
    archive_type: &str, // "monthly" or "yearly"
    show_count: bool,
) -> String {
    let mut archives: HashMap<String, usize> = HashMap::new();

    for post in posts {
        let key = if archive_type == "monthly" {
            post.date.format("%Y/%m").to_string()
        } else {
            post.date.format("%Y").to_string()
        };
        *archives.entry(key).or_insert(0) += 1;
    }

    if archives.is_empty() {
        return String::new();
    }

    let mut html = r#"<ul class="archive-list">"#.to_string();

    let mut sorted: Vec<_> = archives.iter().collect();
    sorted.sort_by(|a, b| b.0.cmp(a.0)); // Descending order

    for (key, count) in sorted {
        let url = url_for(config, &format!("{}/{}/", config.archive_dir, key));

        let display = if archive_type == "monthly" {
            // Parse year/month and format nicely
            let parts: Vec<&str> = key.split('/').collect();
            if parts.len() == 2 {
                format!("{} {}", month_name(parts[1]), parts[0])
            } else {
                key.clone()
            }
        } else {
            key.clone()
        };

        html.push_str(&format!(
            r#"<li class="archive-list-item"><a class="archive-list-link" href="{}">{}</a>"#,
            url, display
        ));

        if show_count {
            html.push_str(&format!(
                r#"<span class="archive-list-count">{}</span>"#,
                count
            ));
        }

        html.push_str("</li>");
    }

    html.push_str("</ul>");
    html
}

/// Generate a list of recent posts
pub fn list_posts(posts: &[Post], amount: usize) -> String {
    let mut html = r#"<ul class="post-list">"#.to_string();

    for post in posts.iter().take(amount) {
        html.push_str(&format!(
            r#"<li class="post-list-item"><a class="post-list-link" href="{}">{}</a></li>"#,
            post.permalink, post.title
        ));
    }

    html.push_str("</ul>");
    html
}

/// Generate a tag cloud
pub fn tagcloud(
    config: &SiteConfig,
    posts: &[Post],
    min_font: f32,
    max_font: f32,
    unit: &str,
) -> String {
    let mut tags: HashMap<String, usize> = HashMap::new();

    for post in posts {
        for tag in &post.tags {
            *tags.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    if tags.is_empty() {
        return String::new();
    }

    let min_count = *tags.values().min().unwrap_or(&1) as f32;
    let max_count = *tags.values().max().unwrap_or(&1) as f32;
    let count_range = (max_count - min_count).max(1.0);

    let mut html = r#"<div class="tagcloud">"#.to_string();

    let mut sorted: Vec<_> = tags.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    for (name, count) in sorted {
        let slug = slug::slugify(name);
        let url = url_for(config, &format!("{}/{}/", config.tag_dir, slug));

        // Calculate font size
        let size = if count_range > 0.0 {
            let ratio = (*count as f32 - min_count) / count_range;
            min_font + ratio * (max_font - min_font)
        } else {
            min_font
        };

        html.push_str(&format!(
            r#"<a href="{}" style="font-size: {:.2}{}">{}</a> "#,
            url, size, unit, name
        ));
    }

    html.push_str("</div>");
    html
}

/// Generate a paginator
pub fn paginator(
    current: usize,
    total: usize,
    base_url: &str,
    pagination_dir: &str,
    prev_text: &str,
    next_text: &str,
    mid_size: usize,
) -> String {
    if total <= 1 {
        return String::new();
    }

    let mut html = r#"<nav class="pagination">"#.to_string();

    // Previous link
    if current > 1 {
        let prev_url = if current == 2 {
            base_url.to_string()
        } else {
            format!("{}{}/{}/", base_url, pagination_dir, current - 1)
        };
        html.push_str(&format!(
            r#"<a class="pagination-prev" href="{}">{}</a>"#,
            prev_url, prev_text
        ));
    } else {
        html.push_str(&format!(
            r#"<span class="pagination-prev disabled">{}</span>"#,
            prev_text
        ));
    }

    // Page numbers
    html.push_str(r#"<span class="pagination-numbers">"#);

    let start = (current.saturating_sub(mid_size)).max(1);
    let end = (current + mid_size).min(total);

    if start > 1 {
        html.push_str(&format!(
            r#"<a class="pagination-number" href="{}">1</a>"#,
            base_url
        ));
        if start > 2 {
            html.push_str(r#"<span class="pagination-ellipsis">…</span>"#);
        }
    }

    for page in start..=end {
        let url = if page == 1 {
            base_url.to_string()
        } else {
            format!("{}{}/{}/", base_url, pagination_dir, page)
        };

        if page == current {
            html.push_str(&format!(
                r#"<span class="pagination-number current">{}</span>"#,
                page
            ));
        } else {
            html.push_str(&format!(
                r#"<a class="pagination-number" href="{}">{}</a>"#,
                url, page
            ));
        }
    }

    if end < total {
        if end < total - 1 {
            html.push_str(r#"<span class="pagination-ellipsis">…</span>"#);
        }
        let url = format!("{}{}/{}/", base_url, pagination_dir, total);
        html.push_str(&format!(
            r#"<a class="pagination-number" href="{}">{}</a>"#,
            url, total
        ));
    }

    html.push_str("</span>");

    // Next link
    if current < total {
        let next_url = format!("{}{}/{}/", base_url, pagination_dir, current + 1);
        html.push_str(&format!(
            r#"<a class="pagination-next" href="{}">{}</a>"#,
            next_url, next_text
        ));
    } else {
        html.push_str(&format!(
            r#"<span class="pagination-next disabled">{}</span>"#,
            next_text
        ));
    }

    html.push_str("</nav>");
    html
}

/// Convert month number to name
fn month_name(month: &str) -> &'static str {
    match month {
        "01" => "January",
        "02" => "February",
        "03" => "March",
        "04" => "April",
        "05" => "May",
        "06" => "June",
        "07" => "July",
        "08" => "August",
        "09" => "September",
        "10" => "October",
        "11" => "November",
        "12" => "December",
        _ => "Unknown",
    }
}

/// Table of contents generator
pub fn toc(content: &str, max_depth: usize) -> String {
    // Simple TOC generator - parse headings from HTML
    let mut html = r#"<ol class="toc">"#.to_string();
    let mut current_level = 0;

    // Simple regex-free heading extraction
    let mut i = 0;
    let chars: Vec<char> = content.chars().collect();

    while i < chars.len() {
        // Look for <h1>, <h2>, etc.
        if chars[i] == '<' && i + 3 < chars.len() && chars[i + 1] == 'h' {
            if let Some(level) = chars[i + 2].to_digit(10) {
                let level = level as usize;
                if level <= max_depth {
                    // Find the closing >
                    if let Some(start) = chars[i..].iter().position(|&c| c == '>') {
                        let start = i + start + 1;
                        // Find </h{level}>
                        let end_tag = format!("</h{}>", level);
                        let end_chars: Vec<char> = end_tag.chars().collect();

                        if let Some(end) = find_sequence(&chars[start..], &end_chars) {
                            let heading: String = chars[start..start + end].iter().collect();
                            let heading = strip_tags(&heading);

                            // Adjust nesting
                            while current_level < level {
                                html.push_str("<ol>");
                                current_level += 1;
                            }
                            while current_level > level {
                                html.push_str("</ol>");
                                current_level -= 1;
                            }

                            let id = slug::slugify(&heading);
                            html.push_str(&format!(
                                "<li class=\"toc-item toc-level-{}\"><a class=\"toc-link\" href=\"#{}\"><span class=\"toc-text\">{}</span></a></li>",
                                level, id, heading
                            ));

                            i = start + end + end_chars.len();
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
    }

    while current_level > 0 {
        html.push_str("</ol>");
        current_level -= 1;
    }

    html.push_str("</ol>");
    html
}

fn find_sequence(haystack: &[char], needle: &[char]) -> Option<usize> {
    'outer: for i in 0..haystack.len() {
        if i + needle.len() > haystack.len() {
            return None;
        }
        for j in 0..needle.len() {
            if haystack[i + j] != needle[j] {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

fn strip_tags(s: &str) -> String {
    let mut result = String::new();
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
