//! Post and Page models

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A blog post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    /// Post title
    pub title: String,

    /// Publication date
    pub date: DateTime<Local>,

    /// Last updated date
    pub updated: Option<DateTime<Local>>,

    /// Raw markdown content
    pub raw: String,

    /// Rendered HTML content
    pub content: String,

    /// Post excerpt (before <!-- more -->)
    pub excerpt: Option<String>,

    /// Content after excerpt
    pub more: Option<String>,

    /// Post tags
    pub tags: Vec<String>,

    /// Post categories (can be hierarchical)
    pub categories: Vec<String>,

    /// Layout template to use
    pub layout: String,

    /// Source file path (relative)
    pub source: String,

    /// Full source file path
    pub full_source: PathBuf,

    /// URL path (without root)
    pub path: String,

    /// Full permalink URL
    pub permalink: String,

    /// Whether comments are enabled
    pub comments: bool,

    /// Whether the post is published
    pub published: bool,

    /// Post language
    pub lang: Option<String>,

    /// Slug (URL-friendly name)
    pub slug: String,

    /// Photos for gallery posts
    pub photos: Vec<String>,

    /// External link for link posts
    pub link: Option<String>,

    /// Custom front-matter fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

impl Post {
    /// Create a new post with minimal required fields
    pub fn new(title: String, date: DateTime<Local>, source: String) -> Self {
        let slug = slug::slugify(&title);
        Self {
            title,
            date,
            updated: None,
            raw: String::new(),
            content: String::new(),
            excerpt: None,
            more: None,
            tags: Vec::new(),
            categories: Vec::new(),
            layout: "post".to_string(),
            source: source.clone(),
            full_source: PathBuf::from(&source),
            path: String::new(),
            permalink: String::new(),
            comments: true,
            published: true,
            lang: None,
            slug,
            photos: Vec::new(),
            link: None,
            extra: HashMap::new(),
        }
    }

    /// Get the previous post in a list
    pub fn prev<'a>(&self, posts: &'a [Post]) -> Option<&'a Post> {
        let pos = posts.iter().position(|p| p.source == self.source)?;
        if pos > 0 {
            Some(&posts[pos - 1])
        } else {
            None
        }
    }

    /// Get the next post in a list
    pub fn next<'a>(&self, posts: &'a [Post]) -> Option<&'a Post> {
        let pos = posts.iter().position(|p| p.source == self.source)?;
        if pos < posts.len() - 1 {
            Some(&posts[pos + 1])
        } else {
            None
        }
    }
}

/// A standalone page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// Page title
    pub title: String,

    /// Creation date
    pub date: DateTime<Local>,

    /// Last updated date
    pub updated: Option<DateTime<Local>>,

    /// Raw markdown content
    pub raw: String,

    /// Rendered HTML content
    pub content: String,

    /// Layout template to use
    pub layout: String,

    /// Source file path (relative)
    pub source: String,

    /// Full source file path
    pub full_source: PathBuf,

    /// URL path (without root)
    pub path: String,

    /// Full permalink URL
    pub permalink: String,

    /// Whether comments are enabled
    pub comments: bool,

    /// Page language
    pub lang: Option<String>,

    /// Custom front-matter fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

impl Page {
    /// Create a new page with minimal required fields
    pub fn new(title: String, date: DateTime<Local>, source: String) -> Self {
        Self {
            title,
            date,
            updated: None,
            raw: String::new(),
            content: String::new(),
            layout: "page".to_string(),
            source: source.clone(),
            full_source: PathBuf::from(&source),
            path: String::new(),
            permalink: String::new(),
            comments: true,
            lang: None,
            extra: HashMap::new(),
        }
    }
}

/// A tag with associated posts
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct Tag {
    pub name: String,
    pub slug: String,
    pub path: String,
    pub permalink: String,
    pub count: usize,
}

#[allow(dead_code)]
impl Tag {
    pub fn new(name: &str, base_url: &str, tag_dir: &str) -> Self {
        let slug = slug::slugify(name);
        let path = format!("{}/{}/", tag_dir, slug);
        let permalink = format!("{}{}", base_url.trim_end_matches('/'), path);
        Self {
            name: name.to_string(),
            slug,
            path,
            permalink,
            count: 0,
        }
    }
}

/// A category with associated posts
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct Category {
    pub name: String,
    pub slug: String,
    pub path: String,
    pub permalink: String,
    pub count: usize,
    /// Parent category for hierarchical categories
    pub parent: Option<String>,
}

#[allow(dead_code)]
impl Category {
    pub fn new(name: &str, base_url: &str, category_dir: &str) -> Self {
        let slug = slug::slugify(name);
        let path = format!("{}/{}/", category_dir, slug);
        let permalink = format!("{}{}", base_url.trim_end_matches('/'), path);
        Self {
            name: name.to_string(),
            slug,
            path,
            permalink,
            count: 0,
            parent: None,
        }
    }
}
