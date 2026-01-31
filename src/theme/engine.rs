//! Template engine abstraction

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::ejs::{EjsContext, EjsValue};

/// Template rendering context
#[derive(Debug, Clone, Default)]
pub struct TemplateContext {
    inner: EjsContext,
}

impl TemplateContext {
    /// Create a new context
    pub fn new() -> Self {
        Self {
            inner: EjsContext::new(),
        }
    }

    /// Set a variable
    pub fn set<V: Into<EjsValue>>(&mut self, name: &str, value: V) {
        self.inner.set(name, value.into());
    }

    /// Set a string variable
    pub fn set_string(&mut self, name: &str, value: &str) {
        self.inner.set_string(name, value);
    }

    /// Set a boolean variable
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.inner.set_bool(name, value);
    }

    /// Set a number variable
    pub fn set_number(&mut self, name: &str, value: f64) {
        self.inner.set_number(name, value);
    }

    /// Set an object variable from a serializable value
    pub fn set_object<T: Serialize>(&mut self, name: &str, value: &T) {
        self.inner.set_object(name, value);
    }

    /// Set a nested object property using dot notation (e.g., "page.prev")
    pub fn set_nested_object<T: Serialize>(&mut self, path: &str, value: &T) {
        self.inner.set_nested_object(path, value);
    }

    /// Get the inner EJS context
    pub fn inner(&self) -> &EjsContext {
        &self.inner
    }

    /// Get mutable inner EJS context
    pub fn inner_mut(&mut self) -> &mut EjsContext {
        &mut self.inner
    }

    /// Convert context to JSON for JavaScript runtime
    pub fn to_json(&self) -> serde_json::Value {
        let json_str = self.inner.to_json();
        serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
    }
}

/// Trait for template engines
pub trait TemplateEngine {
    /// Render a template with the given context
    fn render(&self, template_name: &str, context: &TemplateContext) -> Result<String>;

    /// Render a string template with the given context
    fn render_string(&self, template: &str, context: &TemplateContext) -> Result<String>;

    /// Check if a template exists
    fn has_template(&self, name: &str) -> bool;

    /// Get available template names
    fn template_names(&self) -> Vec<String>;
}

/// Page types for determining which template to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageType {
    Index,
    Post,
    Page,
    Archive,
    Category,
    Tag,
}

impl PageType {
    /// Get the template name for this page type
    pub fn template_name(&self) -> &'static str {
        match self {
            PageType::Index => "index",
            PageType::Post => "post",
            PageType::Page => "page",
            PageType::Archive => "archive",
            PageType::Category => "category",
            PageType::Tag => "tag",
        }
    }

    /// Get fallback template names
    pub fn fallbacks(&self) -> Vec<&'static str> {
        match self {
            PageType::Index => vec![],
            PageType::Post => vec!["index"],
            PageType::Page => vec!["post", "index"],
            PageType::Archive => vec!["index"],
            PageType::Category => vec!["archive", "index"],
            PageType::Tag => vec!["archive", "index"],
        }
    }
}

/// Build context for a post page
pub fn build_post_context(
    post: &crate::content::Post,
    config: &crate::config::SiteConfig,
    site_data: &SiteData,
) -> TemplateContext {
    let mut ctx = TemplateContext::new();

    // Set config
    ctx.set_object("config", config);

    // Set site data
    ctx.set_object("site", site_data);

    // Set page data (the post)
    ctx.set_object("page", post);

    // Set path and url
    ctx.set_string("path", &post.path);
    ctx.set_string("url", &post.permalink);

    ctx
}

/// Build context for an index/archive page
pub fn build_list_context(
    posts: &[crate::content::Post],
    config: &crate::config::SiteConfig,
    site_data: &SiteData,
    page_info: &PaginationInfo,
) -> TemplateContext {
    let mut ctx = TemplateContext::new();

    ctx.set_object("config", config);
    ctx.set_object("site", site_data);
    ctx.set_object("page", page_info);

    // Add posts to page using nested property setter
    let posts_json: Vec<_> = posts
        .iter()
        .map(|p| serde_json::to_value(p).unwrap())
        .collect();
    if let Ok(val) = serde_json::to_value(&posts_json) {
        ctx.inner_mut()
            .set_nested("page.posts", EjsValue::from_json(&val));
    }

    ctx.set_string("path", &page_info.current_url);
    ctx.set_string("url", &page_info.current_url);

    ctx
}

/// Site data available to templates
#[derive(Debug, Clone, Serialize)]
pub struct SiteData {
    pub posts: Vec<PostSummary>,
    pub pages: Vec<PageSummary>,
    pub tags: HashMap<String, usize>,
    pub categories: HashMap<String, usize>,
    /// Total word count of all posts (for statistics)
    #[serde(rename = "wordCount")]
    pub word_count: usize,
}

/// Summary of a post for site data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostSummary {
    pub title: String,
    pub date: String,
    pub path: String,
    pub permalink: String,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub content: String,
    /// Word count for this post
    #[serde(rename = "wordCount")]
    pub word_count: usize,
}

/// Summary of a page for site data
#[derive(Debug, Clone, Serialize)]
pub struct PageSummary {
    pub title: String,
    pub path: String,
    pub permalink: String,
}

/// Pagination information
#[derive(Debug, Clone, Serialize)]
pub struct PaginationInfo {
    pub per_page: usize,
    pub total: usize,
    pub current: usize,
    pub current_url: String,
    pub prev: usize,
    pub prev_link: String,
    pub next: usize,
    pub next_link: String,
    pub is_home: bool,
    pub is_archive: bool,
    pub is_category: bool,
    pub is_tag: bool,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub category: Option<String>,
    pub tag: Option<String>,
}

impl Default for PaginationInfo {
    fn default() -> Self {
        Self {
            per_page: 10,
            total: 1,
            current: 1,
            current_url: "/".to_string(),
            prev: 0,
            prev_link: String::new(),
            next: 0,
            next_link: String::new(),
            is_home: false,
            is_archive: false,
            is_category: false,
            is_tag: false,
            year: None,
            month: None,
            category: None,
            tag: None,
        }
    }
}

/// Archive year data for templates
#[derive(Debug, Clone, Serialize)]
pub struct ArchiveYear {
    pub year: i32,
    pub posts: Vec<PostSummary>,
}
