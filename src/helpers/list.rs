//! Table of contents generator

/// Table of contents generator
pub fn toc(content: &str, max_depth: usize) -> String {
    // Collect all headings first
    let mut headings: Vec<(usize, String, String)> = Vec::new(); // (level, id, text)

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
                            let id = generate_heading_id(&heading);
                            headings.push((level, id, heading));

                            i = start + end + end_chars.len();
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
    }

    if headings.is_empty() {
        return r#"<ol class="toc"></ol>"#.to_string();
    }

    // Find the minimum level to use as base
    let min_level = headings.iter().map(|(l, _, _)| *l).min().unwrap_or(1);

    // Build properly nested TOC
    let mut html = String::new();
    let mut current_level = min_level;

    html.push_str(r#"<ol class="toc">"#);

    for (idx, (level, id, text)) in headings.iter().enumerate() {
        let level = *level;

        // Close child lists and li elements for levels going up
        while current_level > level {
            html.push_str("</ol></li>");
            current_level -= 1;
        }

        // Open child lists for levels going down
        while current_level < level {
            // Don't close the previous li, add child ol inside it
            html.push_str("<ol>");
            current_level += 1;
        }

        // Check if next heading is a child (deeper level)
        let has_children = idx + 1 < headings.len() && headings[idx + 1].0 > level;

        html.push_str(&format!(
            "<li class=\"toc-item toc-level-{}\"><a class=\"toc-link\" href=\"#{}\"><span class=\"toc-text\">{}</span></a>",
            level, id, text
        ));

        // Only close li if no children follow
        if !has_children {
            html.push_str("</li>");
        }
    }

    // Close all remaining levels
    while current_level > min_level {
        html.push_str("</ol></li>");
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

/// Generate heading ID from text (Hexo style)
/// Preserves Chinese characters, replaces spaces with hyphens
fn generate_heading_id(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '-'
            } else if c > '\u{007F}' {
                // Keep non-ASCII characters (Chinese, Japanese, etc.)
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        // Remove consecutive hyphens
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
