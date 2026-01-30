//! Markdown rendering with syntax highlighting

use anyhow::Result;
use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

/// Markdown renderer with syntax highlighting
pub struct MarkdownRenderer {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: String,
    line_numbers: bool,
}

impl MarkdownRenderer {
    /// Create a new markdown renderer
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name: "base16-ocean.dark".to_string(),
            line_numbers: true,
        }
    }

    /// Create with custom settings
    pub fn with_options(theme: &str, line_numbers: bool) -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name: theme.to_string(),
            line_numbers,
        }
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

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(kind)) => {
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
                    code_block_lang = None;
                }
                Event::Text(text)
                    if code_block_lang.is_some() || !code_block_content.is_empty() =>
                {
                    code_block_content.push_str(&text);
                }
                Event::Text(text) if code_block_lang.is_none() && code_block_content.is_empty() => {
                    // Check if we're in a code block context
                    events.push(Event::Text(text));
                }
                _ => {
                    if code_block_lang.is_none() {
                        events.push(event);
                    }
                }
            }
        }

        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        Ok(html_output)
    }

    /// Highlight a code block
    fn highlight_code(&self, code: &str, lang: Option<&str>) -> String {
        let lang = lang.unwrap_or("text");

        // Try to find syntax for the language
        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = self
            .theme_set
            .themes
            .get(&self.theme_name)
            .unwrap_or_else(|| {
                self.theme_set
                    .themes
                    .values()
                    .next()
                    .expect("No themes available")
            });

        match highlighted_html_for_string(code, &self.syntax_set, syntax, theme) {
            Ok(highlighted) => {
                if self.line_numbers {
                    self.add_line_numbers(&highlighted, lang)
                } else {
                    format!(
                        r#"<pre><code class="language-{}">{}</code></pre>"#,
                        lang, highlighted
                    )
                }
            }
            Err(_) => {
                // Fallback to plain code block
                let escaped = html_escape(code);
                format!(
                    r#"<pre><code class="language-{}">{}</code></pre>"#,
                    lang, escaped
                )
            }
        }
    }

    /// Add line numbers to highlighted code
    fn add_line_numbers(&self, code: &str, lang: &str) -> String {
        let lines: Vec<&str> = code.lines().collect();
        let line_count = lines.len();

        let mut gutter = String::new();
        let mut code_lines = String::new();

        for (i, line) in lines.iter().enumerate() {
            gutter.push_str(&format!(r#"<span class="line-number">{}</span>"#, i + 1));
            if i < line_count - 1 {
                gutter.push('\n');
            }

            code_lines.push_str(line);
            if i < line_count - 1 {
                code_lines.push('\n');
            }
        }

        format!(
            r#"<figure class="highlight {}"><table><tr><td class="gutter"><pre>{}</pre></td><td class="code"><pre>{}</pre></td></tr></table></figure>"#,
            lang, gutter, code_lines
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_basic_markdown() {
        let renderer = MarkdownRenderer::new();
        let html = renderer.render("# Hello World\n\nThis is a test.").unwrap();
        assert!(html.contains("<h1>Hello World</h1>"));
        assert!(html.contains("<p>This is a test.</p>"));
    }

    #[test]
    fn test_render_code_block() {
        let renderer = MarkdownRenderer::new();
        let html = renderer.render("```rust\nfn main() {}\n```").unwrap();
        assert!(html.contains("highlight"));
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
