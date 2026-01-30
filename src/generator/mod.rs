//! Generator module - generates static HTML files

use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
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

        // Generate Atom feed
        let start = std::time::Instant::now();
        self.generate_atom_feed(posts)?;
        tracing::debug!("generate_atom_feed took {:?}", start.elapsed());

        // Generate sitemap
        let start = std::time::Instant::now();
        self.generate_sitemap(posts, pages)?;
        tracing::debug!("generate_sitemap took {:?}", start.elapsed());

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
            ctx.set_object("theme", theme.config());
            ctx.set_string("path", &post.path);
            ctx.set_string("url", &post.permalink);

            // Set prev/next posts
            if let Some(prev) = post.prev(all_posts) {
                ctx.set_nested_object("page.prev", prev);
            }
            if let Some(next) = post.next(all_posts) {
                ctx.set_nested_object("page.next", next);
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
            ctx.set_object("theme", theme.config());
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

        let total_pages = posts.len().div_ceil(per_page);

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
                prev: page_num.saturating_sub(1),
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
                ctx.set_object("theme", theme.config());
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
                ctx.set_object("theme", theme.config());
                ctx.set_string("path", &current_url);
                ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));
                theme.render_with_layout("archive", &ctx)?
            } else {
                let posts_vec: Vec<&Post> = year_posts.to_vec();
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
            ctx.set_object("theme", theme.config());
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
        // Use BTreeMap for deterministic iteration order (alphabetically sorted by category name)
        let mut categories: BTreeMap<String, Vec<&Post>> = BTreeMap::new();

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
                ctx.set_object("theme", theme.config());
                ctx.set_string("path", &current_url);
                ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));

                let template = theme
                    .find_template("category", &["archive", "index"])
                    .unwrap_or_else(|| "index".to_string());

                theme.render_with_layout(&template, &ctx)?
            } else {
                let posts_vec: Vec<&Post> = cat_posts.to_vec();
                generate_archive_html(&posts_vec, &format!("Category: {}", category))
            };

            fs::write(output_dir.join("index.html"), html)?;
        }

        Ok(())
    }

    /// Generate tag pages
    fn generate_tags(&self, posts: &[Post], site_data: &SiteData) -> Result<()> {
        // Use BTreeMap for deterministic iteration order (alphabetically sorted by tag name)
        let mut tags: BTreeMap<String, Vec<&Post>> = BTreeMap::new();

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
                ctx.set_object("theme", theme.config());
                ctx.set_string("path", &current_url);
                ctx.set_string("url", &format!("{}{}", self.hexo.config.url, current_url));
                theme.render_with_layout("archive", &ctx)?
            } else {
                let posts_vec: Vec<&Post> = tag_posts.to_vec();
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

    /// Generate Atom feed (atom.xml)
    fn generate_atom_feed(&self, posts: &[Post]) -> Result<()> {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
"#,
        );

        // Site info
        xml.push_str(&format!(
            "  <title>{}</title>\n",
            escape_xml(&self.hexo.config.title)
        ));

        if !self.hexo.config.subtitle.is_empty() {
            xml.push_str(&format!(
                "  <subtitle>{}</subtitle>\n",
                escape_xml(&self.hexo.config.subtitle)
            ));
        }

        xml.push_str(&format!(
            "  <link href=\"{}atom.xml\" rel=\"self\"/>\n",
            self.hexo.config.root
        ));
        xml.push_str(&format!("  <link href=\"{}\"/>\n", self.hexo.config.url));

        // Updated time (use most recent post date or current time)
        let updated = posts
            .first()
            .map(|p| p.updated.unwrap_or(p.date))
            .unwrap_or_else(chrono::Local::now);
        xml.push_str(&format!("  <updated>{}</updated>\n", updated.to_rfc3339()));

        xml.push_str(&format!("  <id>{}/</id>\n", self.hexo.config.url));

        // Author
        xml.push_str("  <author>\n");
        xml.push_str(&format!(
            "    <name>{}</name>\n",
            escape_xml(&self.hexo.config.author)
        ));
        xml.push_str("  </author>\n");

        // Generator
        xml.push_str(
            "  <generator uri=\"https://github.com/ponyma/hexo-rs\">hexo-rs</generator>\n",
        );

        // Entries (limit to most recent 20 posts for feed size)
        let feed_limit = 20;
        for post in posts.iter().take(feed_limit) {
            xml.push_str("  <entry>\n");
            xml.push_str(&format!("    <title>{}</title>\n", escape_xml(&post.title)));
            xml.push_str(&format!(
                "    <link href=\"{}\"/>\n",
                escape_xml(&post.permalink)
            ));
            xml.push_str(&format!("    <id>{}</id>\n", escape_xml(&post.permalink)));
            xml.push_str(&format!(
                "    <published>{}</published>\n",
                post.date.to_rfc3339()
            ));

            let updated = post.updated.unwrap_or(post.date);
            xml.push_str(&format!(
                "    <updated>{}</updated>\n",
                updated.to_rfc3339()
            ));

            // Content (full HTML in CDATA)
            // Convert relative URLs to absolute and escape for CDATA
            let content_with_absolute_urls =
                make_urls_absolute(&post.content, &self.hexo.config.url);
            xml.push_str(&format!(
                "    <content type=\"html\"><![CDATA[{}]]></content>\n",
                escape_cdata(&content_with_absolute_urls)
            ));

            // Summary (excerpt or first 200 chars)
            if let Some(excerpt) = &post.excerpt {
                xml.push_str(&format!(
                    "    <summary type=\"html\">{}</summary>\n",
                    escape_xml(&sanitize_xml_chars(excerpt))
                ));
            }

            // Tags as categories
            for tag in &post.tags {
                let tag_slug = slug::slugify(tag);
                xml.push_str(&format!(
                    "    <category term=\"{}\" scheme=\"{}{}/{}/\"/>\n",
                    escape_xml(tag),
                    self.hexo.config.url,
                    self.hexo.config.tag_dir,
                    tag_slug
                ));
            }

            xml.push_str("  </entry>\n");
        }

        xml.push_str("</feed>\n");

        fs::write(self.hexo.public_dir.join("atom.xml"), xml)?;
        tracing::info!(
            "Generated atom.xml with {} entries",
            posts.len().min(feed_limit)
        );

        Ok(())
    }

    /// Generate sitemap.xml and post-sitemap.xml
    fn generate_sitemap(&self, posts: &[Post], pages: &[Page]) -> Result<()> {
        // Generate main sitemap.xml (simple version)
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
        );

        // Add homepage
        xml.push_str("  <url>\n");
        xml.push_str(&format!("    <loc>{}</loc>\n", self.hexo.config.url));
        if let Some(post) = posts.first() {
            xml.push_str(&format!(
                "    <lastmod>{}</lastmod>\n",
                post.date.format("%Y-%m-%d")
            ));
        }
        xml.push_str("    <changefreq>daily</changefreq>\n");
        xml.push_str("    <priority>1.0</priority>\n");
        xml.push_str("  </url>\n");

        // Add all posts
        for post in posts {
            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", escape_xml(&post.permalink)));
            let lastmod = post.updated.unwrap_or(post.date);
            xml.push_str(&format!(
                "    <lastmod>{}</lastmod>\n",
                lastmod.format("%Y-%m-%d")
            ));
            xml.push_str("    <changefreq>monthly</changefreq>\n");
            xml.push_str("    <priority>0.8</priority>\n");
            xml.push_str("  </url>\n");
        }

        // Add all pages
        for page in pages {
            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", escape_xml(&page.permalink)));
            let lastmod = page.updated.unwrap_or(page.date);
            xml.push_str(&format!(
                "    <lastmod>{}</lastmod>\n",
                lastmod.format("%Y-%m-%d")
            ));
            xml.push_str("    <changefreq>monthly</changefreq>\n");
            xml.push_str("    <priority>0.6</priority>\n");
            xml.push_str("  </url>\n");
        }

        // Add archive page
        xml.push_str("  <url>\n");
        xml.push_str(&format!(
            "    <loc>{}{}/</loc>\n",
            self.hexo.config.url, self.hexo.config.archive_dir
        ));
        xml.push_str("    <changefreq>weekly</changefreq>\n");
        xml.push_str("    <priority>0.5</priority>\n");
        xml.push_str("  </url>\n");

        xml.push_str("</urlset>\n");

        fs::write(self.hexo.public_dir.join("sitemap.xml"), xml)?;

        // Generate post-sitemap.xml (compatible with hexo-sitemap plugin)
        self.generate_post_sitemap(posts)?;

        tracing::info!(
            "Generated sitemap.xml with {} URLs",
            posts.len() + pages.len() + 2
        );

        Ok(())
    }

    /// Generate post-sitemap.xml (compatible with hexo-sitemap plugin format)
    fn generate_post_sitemap(&self, posts: &[Post]) -> Result<()> {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?><?xml-stylesheet type="text/xsl" href="sitemap.xsl"?>
<urlset xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:image="http://www.google.com/schemas/sitemap-image/1.1" xsi:schemaLocation="http://www.sitemaps.org/schemas/sitemap/0.9 http://www.sitemaps.org/schemas/sitemap/0.9/sitemap.xsd" xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">

"#,
        );

        // Add homepage
        xml.push_str("    <url>\n");
        xml.push_str(&format!(
            "        <loc>{}/</loc>\n",
            self.hexo.config.url.trim_end_matches('/')
        ));
        xml.push_str("        <changefreq>daily</changefreq>\n");
        xml.push_str("        <priority>1</priority>\n");
        xml.push_str("    </url>\n\n");

        // Add all posts
        for post in posts {
            let lastmod = post.updated.unwrap_or(post.date);
            xml.push_str("    <url>\n");
            xml.push_str(&format!(
                "        <loc>{}</loc>\n",
                escape_xml(&post.permalink)
            ));
            xml.push_str(&format!(
                "        <lastmod>{}</lastmod>\n",
                lastmod.to_rfc3339()
            ));
            xml.push_str("        <changefreq>weekly</changefreq>\n");
            xml.push_str("        <priority>0.6</priority>\n");
            xml.push_str("\n    </url>\n\n");
        }

        xml.push_str("</urlset>\n");

        fs::write(self.hexo.public_dir.join("post-sitemap.xml"), xml)?;

        // Generate sitemap.xsl stylesheet
        self.generate_sitemap_xsl()?;

        Ok(())
    }

    /// Generate sitemap.xsl stylesheet for pretty display
    fn generate_sitemap_xsl(&self) -> Result<()> {
        let xsl = r#"<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform" xmlns:sitemap="http://www.sitemaps.org/schemas/sitemap/0.9">
<xsl:output method="html" encoding="UTF-8" indent="yes"/>
<xsl:template match="/">
<html>
<head>
<title>XML Sitemap</title>
<style type="text/css">
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 0; padding: 20px; }
h1 { color: #333; border-bottom: 2px solid #4a90d9; padding-bottom: 10px; }
table { border-collapse: collapse; width: 100%; margin-top: 20px; }
th { background: #4a90d9; color: white; padding: 12px; text-align: left; }
td { padding: 10px; border-bottom: 1px solid #ddd; }
tr:hover { background: #f5f5f5; }
a { color: #4a90d9; text-decoration: none; }
a:hover { text-decoration: underline; }
.count { color: #666; margin-bottom: 20px; }
</style>
</head>
<body>
<h1>XML Sitemap</h1>
<p class="count">Number of URLs: <xsl:value-of select="count(sitemap:urlset/sitemap:url)"/></p>
<table>
<tr><th>URL</th><th>Last Modified</th><th>Change Freq</th><th>Priority</th></tr>
<xsl:for-each select="sitemap:urlset/sitemap:url">
<tr>
<td><a href="{sitemap:loc}"><xsl:value-of select="sitemap:loc"/></a></td>
<td><xsl:value-of select="sitemap:lastmod"/></td>
<td><xsl:value-of select="sitemap:changefreq"/></td>
<td><xsl:value-of select="sitemap:priority"/></td>
</tr>
</xsl:for-each>
</table>
</body>
</html>
</xsl:template>
</xsl:stylesheet>
"#;

        fs::write(self.hexo.public_dir.join("sitemap.xsl"), xsl)?;
        Ok(())
    }
}

/// Escape special XML characters
fn escape_xml(s: &str) -> String {
    sanitize_xml_chars(s)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Escape content for use in CDATA sections
/// The only sequence that breaks CDATA is "]]>" - we split it to "]]]]><![CDATA[>"
fn escape_cdata(s: &str) -> String {
    sanitize_xml_chars(s).replace("]]>", "]]]]><![CDATA[>")
}

/// Remove invalid XML characters (control characters except tab, newline, carriage return)
fn sanitize_xml_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            // Valid XML 1.0 characters:
            // #x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF]
            c == '\t'
                || c == '\n'
                || c == '\r'
                || ('\u{0020}'..='\u{D7FF}').contains(&c)
                || ('\u{E000}'..='\u{FFFD}').contains(&c)
                || ('\u{10000}'..='\u{10FFFF}').contains(&c)
        })
        .collect()
}

/// Convert relative URLs in HTML content to absolute URLs
/// Handles src="/..." and href="/..." patterns
fn make_urls_absolute(html: &str, base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    // Handle src="/path" and href="/path" patterns
    let result = html
        .replace("src=\"/", &format!("src=\"{}/", base))
        .replace("href=\"/", &format!("href=\"{}/", base));
    result
}

/// Strip HTML tags from content (for generating plain text summaries)
#[allow(dead_code)]
fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result
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
