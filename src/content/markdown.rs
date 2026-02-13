//! Markdown rendering with syntax highlighting

use anyhow::Result;
use pulldown_cmark::{
    html, CodeBlockKind, CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;

/// Markdown renderer
pub struct MarkdownRenderer {
    syntax_set: SyntaxSet,
}

impl MarkdownRenderer {
    /// Create a new markdown renderer
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
        }
    }

    /// Create with custom settings (kept for API compatibility)
    pub fn with_options(_theme: &str, _line_numbers: bool) -> Self {
        Self::new()
    }

    /// Render markdown to HTML
    pub fn render(&self, markdown: &str) -> Result<String> {
        // Enable most options but NOT YAML metadata blocks
        // We handle front-matter separately in FrontMatter::parse()
        let options = Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_SMART_PUNCTUATION
            | Options::ENABLE_HEADING_ATTRIBUTES
            | Options::ENABLE_DEFINITION_LIST
            | Options::ENABLE_GFM;
        let parser = Parser::new_ext(markdown, options);

        let mut events: Vec<Event> = Vec::new();
        let mut code_block_lang: Option<String> = None;
        let mut code_block_content = String::new();
        let mut in_code_block = false;

        // Track heading state for adding IDs and anchor links
        let mut in_heading: Option<HeadingLevel> = None;
        let mut heading_text = String::new();

        // Track link state for adding target="_blank" to external links
        let mut in_external_link: Option<(String, String)> = None; // (url, title)
        let mut link_text = String::new();

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    in_code_block = true;
                    code_block_lang = match kind {
                        CodeBlockKind::Fenced(lang) => {
                            let lang = lang.to_string();
                            if lang.is_empty() {
                                None
                            } else {
                                Some(lang)
                            }
                        }
                        CodeBlockKind::Indented => None,
                    };
                    code_block_content.clear();
                }
                Event::End(TagEnd::CodeBlock) => {
                    let highlighted =
                        self.highlight_code(&code_block_content, code_block_lang.as_deref());
                    events.push(Event::Html(CowStr::from(highlighted)));
                    in_code_block = false;
                    code_block_lang = None;
                    code_block_content.clear();
                }
                Event::Text(text) if in_code_block => {
                    code_block_content.push_str(&text);
                }
                // Handle heading start - capture the level and prepare to collect text
                Event::Start(Tag::Heading { level, .. }) => {
                    in_heading = Some(level);
                    heading_text.clear();
                }
                // Collect text inside headings
                Event::Text(ref text) if in_heading.is_some() => {
                    heading_text.push_str(text);
                    // Don't push the event yet, we'll create a custom heading
                }
                Event::Code(ref code) if in_heading.is_some() => {
                    heading_text.push_str(code);
                    // Don't push the event yet
                }
                // Handle heading end - generate heading with ID and anchor
                Event::End(TagEnd::Heading(level)) => {
                    if in_heading.is_some() {
                        // Generate ID from heading text (Hexo style: preserve Chinese, replace spaces with -)
                        let id = generate_heading_id(&heading_text);
                        let level_num = heading_level_to_u8(level);

                        // Generate heading HTML like Hexo:
                        let escaped_id = html_escape_attr(&id);
                        let escaped_title = html_escape_attr(&heading_text);
                        let escaped_text = html_escape(&heading_text);
                        let heading_html = format!(
                            "<h{} id=\"{}\"><a href=\"#{}\" class=\"headerlink\" title=\"{}\"></a>{}</h{}>",
                            level_num, escaped_id, escaped_id, escaped_title, escaped_text, level_num
                        );

                        events.push(Event::Html(CowStr::from(heading_html)));
                        in_heading = None;
                        heading_text.clear();
                    }
                }
                // Handle external links - add target="_blank" rel="noopener"
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    ..
                }) => {
                    let url = dest_url.to_string();
                    let is_external = url.starts_with("http://") || url.starts_with("https://");

                    if is_external && link_type != LinkType::Email {
                        in_external_link = Some((url, title.to_string()));
                        link_text.clear();
                    } else {
                        // Internal link, pass through normally
                        events.push(Event::Start(Tag::Link {
                            link_type,
                            dest_url,
                            title,
                            id: CowStr::Borrowed(""),
                        }));
                    }
                }
                // Collect text inside external links
                Event::Text(ref text) if in_external_link.is_some() => {
                    link_text.push_str(text);
                }
                Event::Code(ref code) if in_external_link.is_some() => {
                    link_text.push_str(&format!("<code>{}</code>", html_escape(code)));
                }
                // Handle external link end
                Event::End(TagEnd::Link) => {
                    if let Some((url, title)) = in_external_link.take() {
                        let title_attr = if title.is_empty() {
                            String::new()
                        } else {
                            format!(" title=\"{}\"", html_escape_attr(&title))
                        };
                        let link_html = format!(
                            "<a target=\"_blank\" rel=\"noopener\" href=\"{}\"{}>{}</a>",
                            html_escape_attr(&url),
                            title_attr,
                            link_text
                        );
                        events.push(Event::Html(CowStr::from(link_html)));
                        link_text.clear();
                    } else {
                        events.push(Event::End(TagEnd::Link));
                    }
                }
                _ => {
                    if !in_code_block && in_heading.is_none() && in_external_link.is_none() {
                        events.push(event);
                    }
                }
            }
        }

        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        Ok(html_output)
    }

    /// Highlight a code block - output Prism.js compatible format with syntax highlighting
    fn highlight_code(&self, code: &str, lang: Option<&str>) -> String {
        let lang = lang.unwrap_or("plain");

        // Try to find syntax for the language
        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang));

        let highlighted = if let Some(syntax) = syntax {
            // Use ClassedHTMLGenerator with Prism-compatible class names
            let mut generator = ClassedHTMLGenerator::new_with_class_style(
                syntax,
                &self.syntax_set,
                ClassStyle::Spaced,
            );

            for line in syntect::util::LinesWithEndings::from(code) {
                let _ = generator.parse_html_for_line_which_includes_newline(line);
            }

            let html = generator.finalize();
            // Convert syntect class names to Prism.js token format
            convert_to_prism_tokens(&html)
        } else {
            // No syntax found, just escape the code
            html_escape(code)
        };

        // Output Prism.js compatible format:
        // <pre class="line-numbers language-rust" data-language="rust"><code class="language-rust">...</code></pre>
        format!(
            "<pre class=\"line-numbers language-{}\" data-language=\"{}\"><code class=\"language-{}\">{}</code></pre>",
            lang, lang, lang, highlighted
        )
    }

    /// Parse excerpt from content (split by <!-- more -->)
    pub fn split_excerpt(content: &str) -> (Option<String>, String) {
        if let Some(pos) = content.find("<!-- more -->") {
            let excerpt = content[..pos].trim().to_string();
            let remaining = content[pos + 13..].trim().to_string();
            let full = format!("{}\n\n{}", excerpt, remaining);
            (Some(excerpt), full)
        } else {
            (None, content.to_string())
        }
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple HTML escaping
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// HTML escaping for attributes (also escapes quotes)
fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

/// Convert HeadingLevel to u8
fn heading_level_to_u8(level: pulldown_cmark::HeadingLevel) -> u8 {
    match level {
        pulldown_cmark::HeadingLevel::H1 => 1,
        pulldown_cmark::HeadingLevel::H2 => 2,
        pulldown_cmark::HeadingLevel::H3 => 3,
        pulldown_cmark::HeadingLevel::H4 => 4,
        pulldown_cmark::HeadingLevel::H5 => 5,
        pulldown_cmark::HeadingLevel::H6 => 6,
    }
}

/// Convert syntect class names to Prism.js token format
/// syntect outputs: <span class="source rust"><span class="storage type">fn</span>...
/// Prism expects: <span class="token keyword">fn</span>...
fn convert_to_prism_tokens(html: &str) -> String {
    // Map syntect scope names to Prism token types
    let replacements = [
        // Keywords
        ("class=\"storage type", "class=\"token keyword"),
        ("class=\"storage modifier", "class=\"token keyword"),
        ("class=\"keyword control", "class=\"token keyword"),
        ("class=\"keyword operator", "class=\"token operator"),
        ("class=\"keyword other", "class=\"token keyword"),
        // Types and classes
        ("class=\"entity name type", "class=\"token class-name"),
        ("class=\"entity name class", "class=\"token class-name"),
        ("class=\"entity name function", "class=\"token function"),
        ("class=\"support type", "class=\"token class-name"),
        ("class=\"support class", "class=\"token class-name"),
        ("class=\"support function", "class=\"token function"),
        // Strings
        ("class=\"string quoted", "class=\"token string"),
        ("class=\"string", "class=\"token string"),
        // Numbers
        ("class=\"constant numeric", "class=\"token number"),
        ("class=\"constant language", "class=\"token boolean"),
        ("class=\"constant other", "class=\"token constant"),
        // Comments
        ("class=\"comment line", "class=\"token comment"),
        ("class=\"comment block", "class=\"token comment"),
        ("class=\"comment", "class=\"token comment"),
        // Punctuation
        ("class=\"punctuation", "class=\"token punctuation"),
        // Operators
        ("class=\"keyword operator", "class=\"token operator"),
        // Variables
        ("class=\"variable", "class=\"token variable"),
        // Meta/other - just remove these wrapper spans
        ("class=\"source", "class=\"token"),
        ("class=\"meta", "class=\"token"),
    ];

    let mut result = html.to_string();
    for (from, to) in replacements {
        result = result.replace(from, to);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_basic_markdown() {
        let renderer = MarkdownRenderer::new();
        let html = renderer.render("# Hello World\n\nThis is a test.").unwrap();
        // Headings now include id and anchor link
        assert!(html.contains(r#"<h1 id="Hello-World">"#));
        assert!(html.contains(r#"class="headerlink""#));
        assert!(html.contains("Hello World</h1>"));
        assert!(html.contains("<p>This is a test.</p>"));
    }

    #[test]
    fn test_render_code_block() {
        let renderer = MarkdownRenderer::new();
        let html = renderer.render("```rust\nfn main() {}\n```").unwrap();
        println!("Generated HTML: {}", html);
        // Should output Prism.js compatible format
        assert!(html.contains("line-numbers language-rust"));
        assert!(html.contains("language-rust"));
        // The code content should be present (possibly with span tags around keywords)
        assert!(html.contains("fn") && html.contains("main"));
    }

    #[test]
    fn test_render_code_block_no_language() {
        let renderer = MarkdownRenderer::new();
        let html = renderer.render("```\nsome code here\n```").unwrap();
        println!("Generated HTML: {}", html);
        // Should wrap content in pre/code tags even without a language
        assert!(html.contains("line-numbers language-plain"));
        assert!(html.contains("some code here"));
        // Content should be INSIDE the pre/code block, not outside
        assert!(!html.contains("<p>some code here</p>"));
    }

    #[test]
    fn test_split_excerpt() {
        let content = "This is excerpt.\n<!-- more -->\nThis is more content.";
        let (excerpt, full) = MarkdownRenderer::split_excerpt(content);
        assert_eq!(excerpt, Some("This is excerpt.".to_string()));
        assert!(full.contains("This is excerpt."));
        assert!(full.contains("This is more content."));
    }

    #[test]
    fn test_ideas_format_markdown() {
        let renderer = MarkdownRenderer::new();
        let markdown = r#"---
第一条内容
-- 2026-01-26

---
第二条内容
-- 2025-12-31
"#;
        let html = renderer.render(markdown).unwrap();
        println!("Rendered HTML:\n{}", html);
        assert!(html.contains("第一条内容"), "Should contain first item");
        assert!(html.contains("第二条内容"), "Should contain second item");
    }
}
