//! Content loader - loads posts and pages from source directory

use anyhow::Result;
use chrono::Local;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use super::{FrontMatter, MarkdownRenderer, Page, Post};
use crate::Hexo;

/// Loads content from the source directory
pub struct ContentLoader<'a> {
    hexo: &'a Hexo,
    renderer: MarkdownRenderer,
}

impl<'a> ContentLoader<'a> {
    /// Create a new content loader
    pub fn new(hexo: &'a Hexo) -> Self {
        let renderer =
            MarkdownRenderer::with_options("base16-ocean.dark", hexo.config.highlight.line_number);
        Self { hexo, renderer }
    }

    /// Load all posts from source/_posts
    pub fn load_posts(&self) -> Result<Vec<Post>> {
        let posts_dir = self.hexo.source_dir.join("_posts");
        if !posts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut posts = Vec::new();

        for entry in WalkDir::new(&posts_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && is_markdown_file(path) {
                match self.load_post(path) {
                    Ok(post) => {
                        if post.published || self.hexo.config.render_drafts {
                            posts.push(post);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load post {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort by date descending (newest first)
        posts.sort_by(|a, b| b.date.cmp(&a.date));

        Ok(posts)
    }

    /// Load a single post from a file
    fn load_post(&self, path: &Path) -> Result<Post> {
        let content = fs::read_to_string(path)?;
        let (fm, body) = FrontMatter::parse(&content)?;

        // Get file metadata for dates
        let metadata = fs::metadata(path)?;
        let file_modified = metadata
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<Local>::from(t));

        // Determine dates
        let date = fm
            .parse_date()
            .unwrap_or_else(|| file_modified.unwrap_or_else(Local::now));

        let updated = fm.parse_updated().or(file_modified);

        // Get title from front-matter or filename
        let title = fm.title.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

        // Calculate source path relative to source dir
        let source = path
            .strip_prefix(&self.hexo.source_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Parse categories (handle nested arrays)
        let categories = parse_categories(&fm.categories);

        // Generate slug from filename (not title) - this matches Hexo.js behavior
        // The :title placeholder in permalink uses the filename, not the actual title
        let filename_slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        let slug = filename_slug;
        let permalink_path = self.generate_permalink(&date, &slug, &categories);
        let permalink = format!(
            "{}{}",
            self.hexo.config.url.trim_end_matches('/'),
            permalink_path
        );

        // Split excerpt and render markdown
        let (excerpt_md, full_md) = MarkdownRenderer::split_excerpt(body);
        let content_html = self.renderer.render(&full_md)?;
        let excerpt_html = excerpt_md
            .as_ref()
            .map(|e| self.renderer.render(e).unwrap_or_default());

        let more = if excerpt_md.is_some() {
            let more_content = body.split("<!-- more -->").nth(1).unwrap_or("");
            Some(self.renderer.render(more_content.trim())?)
        } else {
            None
        };

        let mut post = Post::new(title, date, source);
        post.updated = updated;
        post.raw = body.to_string();
        post.content = content_html;
        post.excerpt = excerpt_html;
        post.more = more;
        post.tags = fm.tags;
        post.categories = categories;
        post.layout = fm.layout.unwrap_or_else(|| "post".to_string());
        post.full_source = path.to_path_buf();
        post.path = permalink_path.clone();
        post.permalink = permalink;
        post.comments = fm.comments;
        post.published = fm.published;
        post.lang = fm.lang;
        post.slug = slug;
        post.extra = fm.extra;

        Ok(post)
    }

    /// Load all pages (non-post markdown files)
    pub fn load_pages(&self) -> Result<Vec<Page>> {
        let mut pages = Vec::new();

        for entry in WalkDir::new(&self.hexo.source_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip special directories
            let relative = path.strip_prefix(&self.hexo.source_dir).unwrap_or(path);
            let first_component = relative
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str());

            if let Some(first) = first_component {
                if first.starts_with('_') {
                    continue;
                }
            }

            if path.is_file() && is_markdown_file(path) {
                match self.load_page(path) {
                    Ok(page) => pages.push(page),
                    Err(e) => {
                        tracing::warn!("Failed to load page {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(pages)
    }

    /// Load a single page from a file
    fn load_page(&self, path: &Path) -> Result<Page> {
        let content = fs::read_to_string(path)?;
        let (fm, body) = FrontMatter::parse(&content)?;

        // Get file metadata
        let metadata = fs::metadata(path)?;
        let file_modified = metadata
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<Local>::from(t));

        let date = fm
            .parse_date()
            .unwrap_or_else(|| file_modified.unwrap_or_else(Local::now));

        let updated = fm.parse_updated().or(file_modified);

        let title = fm.title.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

        let source = path
            .strip_prefix(&self.hexo.source_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Generate page path
        // For index.md files, use the parent directory as the path
        let page_path = {
            let without_ext = source.trim_end_matches(".md").trim_end_matches(".markdown");

            // If the file is index.md, use the parent directory path
            if without_ext.ends_with("/index") || without_ext == "index" {
                without_ext.trim_end_matches("index").to_string()
            } else {
                without_ext.to_string() + "/"
            }
        };

        // Ensure path is not empty
        let page_path = if page_path.is_empty() {
            "/".to_string()
        } else {
            page_path
        };

        let permalink = format!(
            "{}{}{}",
            self.hexo.config.url.trim_end_matches('/'),
            self.hexo.config.root,
            page_path.trim_start_matches('/')
        );

        let content_html = self.renderer.render(body)?;

        let mut page = Page::new(title, date, source);
        page.updated = updated;
        page.raw = body.to_string();
        page.content = content_html;
        page.layout = fm.layout.unwrap_or_else(|| "page".to_string());
        page.full_source = path.to_path_buf();
        page.path = page_path;
        page.permalink = permalink;
        page.comments = fm.comments;
        page.lang = fm.lang;
        page.extra = fm.extra;

        Ok(page)
    }

    /// Generate permalink based on config pattern
    fn generate_permalink(
        &self,
        date: &chrono::DateTime<Local>,
        slug: &str,
        categories: &[String],
    ) -> String {
        let pattern = &self.hexo.config.permalink;

        let category = categories
            .first()
            .map(|c| slug::slugify(c))
            .unwrap_or_default();

        let result = pattern
            .replace(":year", &date.format("%Y").to_string())
            .replace(":month", &date.format("%m").to_string())
            .replace(":day", &date.format("%d").to_string())
            .replace(":i_month", &date.format("%-m").to_string())
            .replace(":i_day", &date.format("%-d").to_string())
            .replace(":hour", &date.format("%H").to_string())
            .replace(":minute", &date.format("%M").to_string())
            .replace(":second", &date.format("%S").to_string())
            .replace(":title", slug)
            .replace(":name", slug)
            .replace(":category", &category)
            .replace(":id", slug);

        format!(
            "{}{}",
            self.hexo.config.root,
            result.trim_start_matches('/')
        )
    }
}

/// Check if a file is a markdown file
fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "md" || e == "markdown")
        .unwrap_or(false)
}

/// Parse categories from front-matter (handles nested arrays)
fn parse_categories(categories: &[String]) -> Vec<String> {
    // For now, flatten any nested structure
    categories.to_vec()
}
