//! Generator module - generates static HTML files

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use walkdir::WalkDir;

use crate::content::{Page, Post};
use crate::theme::engine::{PageSummary, PaginationInfo, PostSummary, SiteData, TemplateContext};
use crate::theme::ThemeLoader;
use crate::Hexo;

/// Static site generator
pub struct Generator<'a> {
    hexo: &'a Hexo,
    theme: Option<ThemeLoader>,
}

impl<'a> Generator<'a> {
    /// Create a new generator
    pub fn new(hexo: &'a Hexo) -> Result<Self> {
        let theme = if hexo.theme_dir.exists() {
            Some(ThemeLoader::load(&hexo.theme_dir)?)
        } else {
            tracing::warn!("Theme directory not found: {:?}", hexo.theme_dir);
            None
        };

        Ok(Self { hexo, theme })
    }

    /// Generate the complete site
    pub fn generate(&self, posts: &[Post], pages: &[Page]) -> Result<()> {
        // Create public directory
        fs::create_dir_all(&self.hexo.public_dir)?;

        // Build site data for templates
        let site_data = self.build_site_data(posts, pages);

        // Generate posts
        for post in posts {
            self.generate_post(post, &site_data, posts)?;
        }
        tracing::info!("Generated {} posts", posts.len());

        // Generate pages
        for page in pages {
            self.generate_page(page, &site_data)?;
        }
        tracing::info!("Generated {} pages", pages.len());

        // Generate index pages
        let start = std::time::Instant::now();
        self.generate_index(posts, &site_data)?;
        tracing::debug!("generate_index took {:?}", start.elapsed());

        // Generate archives
        let start = std::time::Instant::now();
        self.generate_archives(posts, &site_data)?;
        tracing::debug!("generate_archives took {:?}", start.elapsed());

        // Generate category pages
        let start = std::time::Instant::now();
        self.generate_categories(posts, &site_data)?;
        tracing::debug!("generate_categories took {:?}", start.elapsed());

        // Generate tag pages
        let start = std::time::Instant::now();
        self.generate_tags(posts, &site_data)?;
        tracing::debug!("generate_tags took {:?}", start.elapsed());

        // Copy static assets from source
        let start = std::time::Instant::now();
        self.copy_source_assets()?;
        tracing::debug!("copy_source_assets took {:?}", start.elapsed());

        // Copy theme assets
        let start = std::time::Instant::now();
        if let Some(theme) = &self.theme {
            theme.copy_source(&self.hexo.public_dir)?;
        }
        tracing::debug!("theme.copy_source took {:?}", start.elapsed());

        Ok(())
    }

    /// Build site data for templates
    fn build_site_data(&self, posts: &[Post], pages: &[Page]) -> SiteData {
        let mut tags: HashMap<String, usize> = HashMap::new();
        let mut categories: HashMap<String, usize> = HashMap::new();

        for post in posts {
            for tag in &post.tags {
                *tags.entry(tag.clone()).or_insert(0) += 1;
            }
            for cat in &post.categories {
                *categories.entry(cat.clone()).or_insert(0) += 1;
            }
        }

        // Calculate word counts
        let post_summaries: Vec<PostSummary> = posts
            .iter()
            .map(|p| {
                let word_count = count_chinese_chars(&p.content);
                PostSummary {
                    title: p.title.clone(),
                    date: p.date.format("%Y-%m-%d").to_string(),
                    path: p.path.clone(),
                    permalink: p.permalink.clone(),
                    tags: p.tags.clone(),
                    categories: p.categories.clone(),
                    content: p.content.clone(),
                    word_count,
                }
            })
            .collect();

        let total_word_count: usize = post_summaries.iter().map(|p| p.word_count).sum();

        SiteData {
            posts: post_summaries,
            pages: pages
                .iter()
                .map(|p| PageSummary {
                    title: p.title.clone(),
                    path: p.path.clone(),
                    permalink: p.permalink.clone(),
                })
                .collect(),
            tags,
            categories,
            word_count: total_word_count,
        }
    }

    /// Generate a single post
    fn generate_post(&self, post: &Post, site_data: &SiteData, all_posts: &[Post]) -> Result<()> {
        let output_path = self.hexo.public_dir.join(post.path.trim_start_matches('/'));
        let output_file = output_path.join("index.html");

        fs::create_dir_all(&output_path)?;

        let html = if let Some(theme) = &self.theme {
            let mut ctx = TemplateContext::new();
            ctx.set_object("config", &self.hexo.config);
            ctx.set_object("site", site_data);
            ctx.set_object("page", post);
            ctx.set_string("path", &post.path);
            ctx.set_string("url", &post.permalink);

            // Set prev/next posts
            if let Some(prev) = post.prev(all_posts) {
                ctx.set_object("page.prev", prev);
            }
            if let Some(next) = post.next(all_posts) {
                ctx.set_object("page.next", next);
            }

            // Find template
            let template = theme
                .find_template(&post.layout, &["post", "page", "index"])
                .unwrap_or_else(|| "index".to_string());

            theme.render_with_layout(&template, &ctx)?
        } else {
            // No theme - generate basic HTML
            generate_basic_html(&post.title, &post.content)
        };

        fs::write(&output_file, html)?;
        tracing::debug!("Generated: {:?}", output_file);

        Ok(())
    }

    /// Generate a single page
    fn generate_page(&self, page: &Page, site_data: &SiteData) -> Result<()> {
        let output_path = self.hexo.public_dir.join(page.path.trim_start_matches('/'));
        let output_file = output_path.join("index.html");

        fs::create_dir_all(&output_path)?;

        let html = if let Some(theme) = &self.theme {
            let mut ctx = TemplateContext::new();
            ctx.set_object("config", &self.hexo.config);
            ctx.set_object("site", site_data);
            ctx.set_object("page", page);
            ctx.set_string("path", &page.path);
            ctx.set_string("url", &page.permalink);

            let template = theme
                .find_template(&page.layout, &["page", "post", "index"])
                .unwrap_or_else(|| "index".to_string());

            theme.render_with_layout(&template, &ctx)?
        } else {
            generate_basic_html(&page.title, &page.content)
        };

        fs::write(&output_file, html)?;
        tracing::debug!("Generated: {:?}", output_file);

        Ok(())
    }

    /// Generate index pages with pagination
    fn generate_index(&self, posts: &[Post], site_data: &SiteData) -> Result<()> {
        let per_page = self.hexo.config.index_generator.per_page;
        if per_page == 0 {
            return Ok(());
        }

        let total_pages = (posts.len() + per_page - 1) / per_page;

        for page_num in 1..=total_pages {
            let start = (page_num - 1) * per_page;
            let end = (start + per_page).min(posts.len());
            let page_posts = &posts[start..end];

            let (output_path, current_url) = if page_num == 1 {
                (self.hexo.public_dir.clone(), self.hexo.config.root.clone())
            } else {
                let path = self
                    .hexo
                    .public_dir
                    .join(&self.hexo.config.pagination_dir)
                    .join(page_num.to_string());
                let url = format!(
                    "{}{}/{}/",
                    self.hexo.config.root, self.hexo.config.pagination_dir, page_num
                );
                (path, url)
            };

            fs::create_dir_all(&output_path)?;

            let page_info = PaginationInfo {
                per_page,
                total: total_pages,
                current: page_num,
                current_url: current_url.clone(),
                prev: if page_num > 1 { page_num - 1 } else { 0 },
                prev_link: if page_num > 1 {
                    if page_num == 2 {
                        self.hexo.config.root.clone()
                    } else {
                        format!(
                            "{}{}/{}/",
                            self.hexo.config.root,
                            self.hexo.config.pagination_dir,
                            page_num - 1
                        )
                    }
                } else {
                    String::new()
                },
                next: if page_num < total_pages {
                    page_num + 1
                } else {
                    0
                },
                next_link: if page_num < total_pages {
                    format!(
                        "{}{}/{}/",
                        self.hexo.config.root,
                        self.hexo.config.pagination_dir,
                        page_num + 1
                    )
                } else {
                    String::new()
                },
                is_home: page_num == 1,
                ..Default::default()
            };

            let html = if let Some(theme) = &self.theme {
                let mut ctx = TemplateContext::new();
                ctx.set_object("config", &self.hexo.config);
                ctx.set_object("site", site_data);
                ctx.set_object("page", &page_info);
                ctx.set_string("path", &current_url);
                ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));
                // Pre-set wordCount for templates that use complex JS expressions
                ctx.set_number("wordCount", site_data.word_count as f64);

                // Add posts to page context using nested setter
                let posts_data: Vec<_> = page_posts.iter().collect();
                ctx.inner_mut().set_nested_object("page.posts", &posts_data);

                theme.render_with_layout("index", &ctx)?
            } else {
                generate_index_html(page_posts, page_num, total_pages)
            };

            fs::write(output_path.join("index.html"), html)?;
        }

        tracing::info!("Generated {} index pages", total_pages);
        Ok(())
    }

    /// Generate archive pages
    fn generate_archives(&self, posts: &[Post], site_data: &SiteData) -> Result<()> {
        let archive_dir = self.hexo.public_dir.join(&self.hexo.config.archive_dir);
        fs::create_dir_all(&archive_dir)?;

        // Group posts by year
        let mut years: HashMap<i32, Vec<&Post>> = HashMap::new();
        for post in posts {
            let year = post.date.format("%Y").to_string().parse().unwrap_or(2024);
            years.entry(year).or_default().push(post);
        }

        // Generate yearly archives with filtered site data
        for (year, year_posts) in &years {
            let year_dir = archive_dir.join(year.to_string());
            fs::create_dir_all(&year_dir)?;

            let page_info = PaginationInfo {
                is_archive: true,
                year: Some(*year),
                current_url: format!(
                    "{}{}/{}/",
                    self.hexo.config.root, self.hexo.config.archive_dir, year
                ),
                ..Default::default()
            };

            let html = if let Some(theme) = &self.theme {
                // Build filtered site data with only this year's posts
                let filtered_posts: Vec<PostSummary> = year_posts
                    .iter()
                    .map(|post| PostSummary {
                        title: post.title.clone(),
                        date: post.date.format("%Y-%m-%d").to_string(),
                        path: post.path.clone(),
                        permalink: post.permalink.clone(),
                        tags: post.tags.clone(),
                        categories: post.categories.clone(),
                        content: post.content.clone(),
                        word_count: post.content.split_whitespace().count(),
                    })
                    .collect();

                let filtered_site = SiteData {
                    posts: filtered_posts,
                    pages: site_data.pages.clone(),
                    tags: site_data.tags.clone(),
                    categories: site_data.categories.clone(),
                    word_count: site_data.word_count,
                };

                let current_url = page_info.current_url.clone();
                let mut ctx = TemplateContext::new();
                ctx.set_object("config", &self.hexo.config);
                ctx.set_object("site", &filtered_site);
                ctx.set_object("page", &page_info);
                ctx.set_string("path", &current_url);
                ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));
                theme.render_with_layout("archive", &ctx)?
            } else {
                let posts_vec: Vec<&Post> = year_posts.iter().copied().collect();
                generate_archive_html(&posts_vec, &format!("Archive: {}", year))
            };

            fs::write(year_dir.join("index.html"), html)?;
        }

        // Generate main archive page - pass all posts
        let current_url = format!("{}{}/", self.hexo.config.root, self.hexo.config.archive_dir);
        let page_info = PaginationInfo {
            is_archive: true,
            current_url: current_url.clone(),
            ..Default::default()
        };

        let html = if let Some(theme) = &self.theme {
            let mut ctx = TemplateContext::new();
            ctx.set_object("config", &self.hexo.config);
            ctx.set_object("site", site_data);
            ctx.set_object("page", &page_info);
            ctx.set_string("path", &current_url);
            ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));
            theme.render_with_layout("archive", &ctx)?
        } else {
            generate_archive_html(&posts.iter().collect::<Vec<_>>(), "Archives")
        };

        fs::write(archive_dir.join("index.html"), html)?;

        Ok(())
    }

    /// Generate category pages
    fn generate_categories(&self, posts: &[Post], site_data: &SiteData) -> Result<()> {
        let mut categories: HashMap<String, Vec<&Post>> = HashMap::new();

        for post in posts {
            for cat in &post.categories {
                categories.entry(cat.clone()).or_default().push(post);
            }
        }

        let cat_dir = self.hexo.public_dir.join(&self.hexo.config.category_dir);

        for (category, cat_posts) in &categories {
            let slug = slug::slugify(category);
            let output_dir = cat_dir.join(&slug);
            fs::create_dir_all(&output_dir)?;

            let current_url = format!(
                "{}{}/{}/",
                self.hexo.config.root, self.hexo.config.category_dir, slug
            );
            let page_info = PaginationInfo {
                is_category: true,
                category: Some(category.clone()),
                current_url: current_url.clone(),
                ..Default::default()
            };

            let html = if let Some(theme) = &self.theme {
                let mut ctx = TemplateContext::new();
                ctx.set_object("config", &self.hexo.config);
                ctx.set_object("site", site_data);
                ctx.set_object("page", &page_info);
                ctx.set_object("page.posts", cat_posts);
                ctx.set_string("path", &current_url);
                ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));

                let template = theme
                    .find_template("category", &["archive", "index"])
                    .unwrap_or_else(|| "index".to_string());

                theme.render_with_layout(&template, &ctx)?
            } else {
                let posts_vec: Vec<&Post> = cat_posts.iter().copied().collect();
                generate_archive_html(&posts_vec, &format!("Category: {}", category))
            };

            fs::write(output_dir.join("index.html"), html)?;
        }

        Ok(())
    }

    /// Generate tag pages
    fn generate_tags(&self, posts: &[Post], site_data: &SiteData) -> Result<()> {
        let mut tags: HashMap<String, Vec<&Post>> = HashMap::new();

        for post in posts {
            for tag in &post.tags {
                tags.entry(tag.clone()).or_default().push(post);
            }
        }

        let tag_dir = self.hexo.public_dir.join(&self.hexo.config.tag_dir);

        for (tag, tag_posts) in &tags {
            let slug = slug::slugify(tag);
            let output_dir = tag_dir.join(&slug);
            fs::create_dir_all(&output_dir)?;

            let current_url = format!(
                "{}{}/{}/",
                self.hexo.config.root, self.hexo.config.tag_dir, slug
            );
            let page_info = PaginationInfo {
                is_tag: true,
                tag: Some(tag.clone()),
                current_url: current_url.clone(),
                ..Default::default()
            };

            // Create a filtered SiteData with only posts for this tag
            // This is the key optimization: instead of passing all 258 posts to the template
            // and letting JS filter them, we pass only the relevant posts
            // The template logic remains unchanged, but processes much less data
            let html = if let Some(theme) = &self.theme {
                // Build filtered site data with only this tag's posts
                let filtered_posts: Vec<PostSummary> = tag_posts
                    .iter()
                    .map(|post| PostSummary {
                        title: post.title.clone(),
                        date: post.date.format("%Y-%m-%d").to_string(),
                        path: post.path.clone(),
                        permalink: post.permalink.clone(),
                        tags: post.tags.clone(),
                        categories: post.categories.clone(),
                        content: post.content.clone(),
                        word_count: post.content.split_whitespace().count(),
                    })
                    .collect();

                let filtered_site = SiteData {
                    posts: filtered_posts,
                    pages: site_data.pages.clone(),
                    tags: site_data.tags.clone(),
                    categories: site_data.categories.clone(),
                    word_count: site_data.word_count,
                };

                // Use archive template with filtered data
                let mut ctx = TemplateContext::new();
                ctx.set_object("config", &self.hexo.config);
                ctx.set_object("site", &filtered_site);
                ctx.set_object("page", &page_info);
                ctx.set_string("path", &current_url);
                ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));
                theme.render_with_layout("archive", &ctx)?
            } else {
                let posts_vec: Vec<&Post> = tag_posts.iter().copied().collect();
                generate_archive_html(&posts_vec, &format!("Tag: {}", tag))
            };

            fs::write(output_dir.join("index.html"), html)?;
        }

        Ok(())
    }

    /// Copy static assets from source directory
    fn copy_source_assets(&self) -> Result<()> {
        for entry in WalkDir::new(&self.hexo.source_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Skip special directories and markdown files
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

            // Skip markdown files (already processed)
            let ext = path.extension().and_then(|e| e.to_str());
            if matches!(ext, Some("md") | Some("markdown")) {
                continue;
            }

            if path.is_file() {
                let dest = self.hexo.public_dir.join(relative);
                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(path, &dest)?;
                tracing::debug!("Copied: {:?}", relative);
            }
        }

        Ok(())
    }
}

/// Generate basic HTML without a theme
fn generate_basic_html(title: &str, content: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; line-height: 1.6; }}
        pre {{ background: #f5f5f5; padding: 15px; overflow-x: auto; }}
        code {{ background: #f5f5f5; padding: 2px 5px; }}
        pre code {{ background: none; padding: 0; }}
    </style>
</head>
<body>
    <article>
        <h1>{}</h1>
        {}
    </article>
</body>
</html>"#,
        title, title, content
    )
}

/// Generate basic index HTML without a theme
fn generate_index_html(posts: &[Post], current_page: usize, total_pages: usize) -> String {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Home</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; line-height: 1.6; }
        .post-list { list-style: none; padding: 0; }
        .post-item { margin-bottom: 30px; padding-bottom: 20px; border-bottom: 1px solid #eee; }
        .post-title { margin: 0 0 10px; }
        .post-date { color: #666; font-size: 0.9em; }
        .pagination { margin-top: 30px; text-align: center; }
    </style>
</head>
<body>
    <ul class="post-list">"#,
    );

    for post in posts {
        html.push_str(&format!(
            r#"<li class="post-item">
    <h2 class="post-title"><a href="{}">{}</a></h2>
    <span class="post-date">{}</span>
    {}
</li>"#,
            post.permalink,
            post.title,
            post.date.format("%Y-%m-%d"),
            post.excerpt.as_deref().unwrap_or("")
        ));
    }

    html.push_str("</ul>");

    if total_pages > 1 {
        html.push_str(r#"<div class="pagination">"#);
        html.push_str(&format!("Page {} of {}", current_page, total_pages));
        html.push_str("</div>");
    }

    html.push_str("</body></html>");
    html
}

/// Generate basic archive HTML without a theme
fn generate_archive_html(posts: &[&Post], title: &str) -> String {
    let mut html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; line-height: 1.6; }}
        .archive-list {{ list-style: none; padding: 0; }}
        .archive-item {{ padding: 10px 0; border-bottom: 1px solid #eee; }}
        .archive-date {{ color: #666; margin-right: 15px; }}
    </style>
</head>
<body>
    <h1>{}</h1>
    <ul class="archive-list">"#,
        title, title
    );

    for post in posts {
        html.push_str(&format!(
            r#"<li class="archive-item"><span class="archive-date">{}</span><a href="{}">{}</a></li>"#,
            post.date.format("%Y-%m-%d"),
            post.permalink,
            post.title
        ));
    }

    html.push_str("</ul></body></html>");
    html
}

/// Count Chinese characters in a string
/// This matches the common CJK Unified Ideographs ranges
fn count_chinese_chars(content: &str) -> usize {
    content
        .chars()
        .filter(|c| {
            // CJK Unified Ideographs (common Chinese characters)
            let code = *c as u32;
            (0x4E00..=0x9FFF).contains(&code) ||    // CJK Unified Ideographs
        (0x3400..=0x4DBF).contains(&code) ||    // CJK Unified Ideographs Extension A
        (0x20000..=0x2A6DF).contains(&code) ||  // CJK Unified Ideographs Extension B
        (0x2A700..=0x2B73F).contains(&code) ||  // CJK Unified Ideographs Extension C
        (0x2B740..=0x2B81F).contains(&code) ||  // CJK Unified Ideographs Extension D
        (0x9FA6..=0x9FCB).contains(&code) // Additional CJK
        })
        .count()
}
