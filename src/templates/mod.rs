//! Built-in vexo theme templates using Tera template engine
//!
//! All templates from the vexo theme are embedded directly in the binary,
//! eliminating the need for QuickJS/EJS runtime.

use anyhow::Result;
use serde::Serialize;
use std::collections::HashMap;
use tera::{Context, Tera};

/// Template renderer with embedded vexo theme
pub struct TemplateRenderer {
    tera: Tera,
}

impl TemplateRenderer {
    /// Create a new renderer with all vexo templates loaded
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();

        // Disable autoescaping for HTML templates since we're generating HTML
        // and URLs/paths should not be escaped
        tera.autoescape_on(vec![]);

        // Register all templates
        tera.add_raw_templates(vec![
            ("layout.html", include_str!("vexo/layout.html")),
            ("index.html", include_str!("vexo/index.html")),
            ("page.html", include_str!("vexo/page.html")),
            ("archive.html", include_str!("vexo/archive.html")),
            ("tags.html", include_str!("vexo/tags.html")),
            ("tag_single.html", include_str!("vexo/tag_single.html")),
            ("search.html", include_str!("vexo/search.html")),
            ("about.html", include_str!("vexo/about.html")),
            ("links.html", include_str!("vexo/links.html")),
            ("project.html", include_str!("vexo/project.html")),
            ("home.html", include_str!("vexo/home.html")),
            // Partials
            (
                "partials/head.html",
                include_str!("vexo/partials/head.html"),
            ),
            (
                "partials/header.html",
                include_str!("vexo/partials/header.html"),
            ),
            (
                "partials/footer.html",
                include_str!("vexo/partials/footer.html"),
            ),
            ("partials/top.html", include_str!("vexo/partials/top.html")),
            ("partials/nav.html", include_str!("vexo/partials/nav.html")),
            (
                "partials/pager.html",
                include_str!("vexo/partials/pager.html"),
            ),
            (
                "partials/catalog.html",
                include_str!("vexo/partials/catalog.html"),
            ),
            ("partials/tag.html", include_str!("vexo/partials/tag.html")),
            (
                "partials/archive_section.html",
                include_str!("vexo/partials/archive_section.html"),
            ),
        ])?;

        // Register custom filters
        tera.register_filter("strip_html", strip_html_filter);
        tera.register_filter("truncate_chars", truncate_chars_filter);
        tera.register_filter("date_format", date_format_filter);

        Ok(Self { tera })
    }

    /// Render a template with given context
    pub fn render(&self, template_name: &str, context: &Context) -> Result<String> {
        Ok(self.tera.render(template_name, context)?)
    }
}

/// Tera filter: strip HTML tags
fn strip_html_filter(
    value: &tera::Value,
    _args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let s = tera::try_get_value!("strip_html", "value", String, value);
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
    Ok(tera::Value::String(result))
}

/// Tera filter: truncate by character count
fn truncate_chars_filter(
    value: &tera::Value,
    args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let s = tera::try_get_value!("truncate_chars", "value", String, value);
    let length = match args.get("length") {
        Some(val) => tera::try_get_value!("truncate_chars", "length", usize, val),
        None => 150,
    };
    let omission = match args.get("omission") {
        Some(val) => tera::try_get_value!("truncate_chars", "omission", String, val),
        None => " .....".to_string(),
    };

    if s.chars().count() <= length {
        Ok(tera::Value::String(s))
    } else {
        let truncated: String = s.chars().take(length).collect();
        Ok(tera::Value::String(format!(
            "{}{}",
            truncated.trim_end(),
            omission
        )))
    }
}

/// Tera filter: format date string
fn date_format_filter(
    value: &tera::Value,
    args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let s = tera::try_get_value!("date_format", "value", String, value);
    let format = match args.get("format") {
        Some(val) => tera::try_get_value!("date_format", "format", String, val),
        None => "YYYY-MM-DD".to_string(),
    };

    // The date is already a formatted string like "2023-05-30"
    // For "LL" format (like "May 30, 2023"), we parse and reformat
    if format == "LL" {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
            return Ok(tera::Value::String(date.format("%B %d, %Y").to_string()));
        }
    }

    // Default: return as-is (already YYYY-MM-DD)
    Ok(tera::Value::String(s))
}

/// Data structures for template context

#[derive(Debug, Clone, Serialize)]
pub struct SiteData {
    pub posts: Vec<PostData>,
    pub pages: Vec<PageData>,
    pub tags: HashMap<String, usize>,
    pub categories: HashMap<String, usize>,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostData {
    pub title: String,
    pub date: String,
    pub path: String,
    pub permalink: String,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub content: String,
    pub excerpt: Option<String>,
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageData {
    pub title: String,
    pub date: String,
    pub path: String,
    pub permalink: String,
    pub content: String,
    pub layout: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginationData {
    pub per_page: usize,
    pub total: usize,
    pub current: usize,
    pub current_url: String,
    pub prev: usize,
    pub prev_link: String,
    pub next: usize,
    pub next_link: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NavPost {
    pub title: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveYearData {
    pub year: i32,
    pub posts: Vec<PostData>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagData {
    pub name: String,
    pub posts: Vec<PostData>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigData {
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub author: String,
    pub url: String,
    pub root: String,
    pub tag_dir: String,
    pub archive_dir: String,
    pub category_dir: String,
    pub per_page: usize,
    pub github_username: String,
    pub keyword: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeData {
    pub description: String,
    pub keyword: String,
    pub excerpt_link: String,
    pub catalog: bool,
    pub qrcode: bool,
    pub menu: Vec<MenuItem>,
    pub about: AboutData,
    pub mathjax_enable: bool,
    pub mathjax_cdn: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MenuItem {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AboutData {
    pub banner: String,
    pub github_username: String,
    pub twitter_username: String,
}
