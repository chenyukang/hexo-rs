//! Generator module - generates static HTML files using built-in Tera templates

use anyhow::Result;
use std::collections::{BTreeMap, HashMap};
use std::fs;

use tera::Context;
use walkdir::WalkDir;

use crate::content::{Page, Post};
use crate::helpers::toc;
use crate::templates::{
    AboutData, ArchiveYearData, ConfigData, MenuItem, NavPost, PaginationData, PostData, SiteData,
    TagData, TemplateRenderer, ThemeData,
};
use crate::theme::ThemeLoader;
use crate::Hexo;

/// Static site generator using Tera templates
pub struct Generator {
    hexo: Hexo,
    renderer: TemplateRenderer,
    theme_loader: ThemeLoader,
}

impl Generator {
    /// Create a new generator
    pub fn new(hexo: &Hexo) -> Result<Self> {
        let renderer = TemplateRenderer::new()?;
        let theme_loader = ThemeLoader::load(&hexo.theme_dir)?;

        Ok(Self {
            hexo: hexo.clone(),
            renderer,
            theme_loader,
        })
    }

    /// Generate the entire site
    pub fn generate(&self, posts: &[Post], pages: &[Page]) -> Result<()> {
        // Ensure public directory exists
        fs::create_dir_all(&self.hexo.public_dir)?;

        // Copy theme assets
        self.theme_loader.copy_source(&self.hexo.public_dir)?;

        // Copy source assets (images, etc.)
        self.copy_source_assets()?;

        // Sort posts by date (newest first)
        let mut sorted_posts: Vec<_> = posts.to_vec();
        sorted_posts.sort_by(|a, b| b.date.cmp(&a.date));

        // Build site data
        let site_data = self.build_site_data(&sorted_posts, pages);

        // Build config data
        let config_data = self.build_config_data();

        // Build theme data
        let theme_data = self.build_theme_data();

        // Generate index pages (with pagination)
        self.generate_index_pages(&sorted_posts, &site_data, &config_data, &theme_data)?;

        // Generate post pages
        self.generate_post_pages(&sorted_posts, &site_data, &config_data, &theme_data)?;

        // Generate standalone pages
        self.generate_page_pages(pages, &site_data, &config_data, &theme_data)?;

        // Generate archive page
        self.generate_archive_page(&sorted_posts, &site_data, &config_data, &theme_data)?;

        // Generate tag pages
        self.generate_tag_pages(&sorted_posts, &site_data, &config_data, &theme_data)?;

        // Generate RSS feed
        self.generate_atom_feed(&sorted_posts)?;

        // Generate search index
        self.generate_search_index(&sorted_posts)?;

        Ok(())
    }

    /// Build site data for templates
    fn build_site_data(&self, posts: &[Post], pages: &[Page]) -> SiteData {
        let mut tags: HashMap<String, usize> = HashMap::new();
        let mut categories: HashMap<String, usize> = HashMap::new();
        let mut total_word_count = 0;

        let post_data: Vec<PostData> = posts
            .iter()
            .map(|p| {
                for tag in &p.tags {
                    *tags.entry(tag.clone()).or_insert(0) += 1;
                }
                for cat in &p.categories {
                    *categories.entry(cat.clone()).or_insert(0) += 1;
                }

                let word_count = count_words(&p.content);
                total_word_count += word_count;

                PostData {
                    title: p.title.clone(),
                    date: p.date.format("%Y-%m-%d").to_string(),
                    path: format!("/{}", p.path.trim_start_matches('/')),
                    permalink: p.permalink.clone(),
                    tags: p.tags.clone(),
                    categories: p.categories.clone(),
                    content: p.content.clone(),
                    excerpt: p.excerpt.clone(),
                    word_count,
                }
            })
            .collect();

        let page_data = pages
            .iter()
            .map(|p| crate::templates::PageData {
                title: p.title.clone(),
                date: p.date.format("%Y-%m-%d").to_string(),
                path: format!("/{}", p.path.trim_start_matches('/')),
                permalink: p.permalink.clone(),
                content: p.content.clone(),
                layout: p.layout.clone(),
            })
            .collect();

        SiteData {
            posts: post_data,
            pages: page_data,
            tags,
            categories,
            word_count: total_word_count,
        }
    }

    /// Build config data for templates
    fn build_config_data(&self) -> ConfigData {
        ConfigData {
            title: self.hexo.config.title.clone(),
            subtitle: self.hexo.config.subtitle.clone(),
            description: self.hexo.config.description.clone(),
            author: self.hexo.config.author.clone(),
            url: self.hexo.config.url.clone(),
            root: self.hexo.config.root.clone(),
            tag_dir: self.hexo.config.tag_dir.clone(),
            archive_dir: self.hexo.config.archive_dir.clone(),
            category_dir: self.hexo.config.category_dir.clone(),
            per_page: self.hexo.config.per_page,
            github_username: self
                .hexo
                .config
                .extra
                .get("github_username")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            keyword: self
                .hexo
                .config
                .keywords
                .as_ref()
                .map(|k| k.join(", "))
                .unwrap_or_default(),
        }
    }

    /// Build theme data for templates
    fn build_theme_data(&self) -> ThemeData {
        let theme_config = self.theme_loader.config();

        // Parse menu items
        let menu: Vec<MenuItem> = theme_config
            .get("menu")
            .and_then(|v| {
                if let serde_yaml::Value::Mapping(map) = v {
                    Some(
                        map.iter()
                            .filter_map(|(k, v)| {
                                let name = k.as_str()?;
                                let path = v.as_str()?;
                                Some(MenuItem {
                                    name: name.to_string(),
                                    path: path.to_string(),
                                })
                            })
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Parse about section
        let about = theme_config
            .get("about")
            .and_then(|v| {
                if let serde_yaml::Value::Mapping(map) = v {
                    Some(AboutData {
                        banner: map
                            .get("banner")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        github_username: map
                            .get("github_username")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        twitter_username: map
                            .get("twitter_username")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                } else {
                    None
                }
            })
            .unwrap_or(AboutData {
                banner: String::new(),
                github_username: String::new(),
                twitter_username: String::new(),
            });

        ThemeData {
            description: theme_config
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            keyword: theme_config
                .get("keyword")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            excerpt_link: theme_config
                .get("excerpt_link")
                .and_then(|v| v.as_str())
                .unwrap_or("Read More")
                .to_string(),
            catalog: theme_config
                .get("catalog")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            qrcode: theme_config
                .get("qrcode")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            menu,
            about,
            mathjax_enable: theme_config
                .get("mathjax_enable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            mathjax_cdn: theme_config
                .get("mathjax_cdn")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            comment: theme_config
                .get("comment")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }

    /// Create a base context with common variables
    fn create_base_context(
        &self,
        site_data: &SiteData,
        config_data: &ConfigData,
        theme_data: &ThemeData,
    ) -> Context {
        let mut context = Context::new();
        context.insert("site", site_data);
        context.insert("config", config_data);
        context.insert("theme", theme_data);
        // Always use Beijing time (UTC+8) for "最近更新"
        let beijing_now =
            chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
        context.insert("current_year", &beijing_now.format("%Y").to_string());
        context.insert("now_formatted", &format_datetime_chinese(&beijing_now));
        context
    }

    /// Generate index pages with pagination
    fn generate_index_pages(
        &self,
        posts: &[Post],
        site_data: &SiteData,
        config_data: &ConfigData,
        theme_data: &ThemeData,
    ) -> Result<()> {
        let per_page = self.hexo.config.per_page;
        let total_pages = posts.len().div_ceil(per_page);

        for page_num in 1..=total_pages {
            let start = (page_num - 1) * per_page;
            let end = (start + per_page).min(posts.len());
            let page_posts: Vec<PostData> = posts[start..end]
                .iter()
                .map(|p| PostData {
                    title: p.title.clone(),
                    date: p.date.format("%Y-%m-%d").to_string(),
                    path: format!("/{}", p.path.trim_start_matches('/')),
                    permalink: p.permalink.clone(),
                    tags: p.tags.clone(),
                    categories: p.categories.clone(),
                    content: p.content.clone(),
                    excerpt: p.excerpt.clone(),
                    word_count: count_words(&p.content),
                })
                .collect();

            let pagination = PaginationData {
                per_page,
                total: total_pages,
                current: page_num,
                current_url: if page_num == 1 {
                    "/".to_string()
                } else {
                    format!("/page/{}/", page_num)
                },
                prev: page_num.saturating_sub(1),
                prev_link: if page_num > 1 {
                    if page_num == 2 {
                        "/".to_string()
                    } else {
                        format!("/page/{}/", page_num - 1)
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
                    format!("/page/{}/", page_num + 1)
                } else {
                    String::new()
                },
            };

            let mut context = self.create_base_context(site_data, config_data, theme_data);
            context.insert("page_posts", &page_posts);
            context.insert("pagination", &pagination);
            context.insert("is_home", &true);
            context.insert("current_path", &pagination.current_url);

            let html = self.renderer.render("index.html", &context)?;

            let output_path = if page_num == 1 {
                self.hexo.public_dir.join("index.html")
            } else {
                self.hexo
                    .public_dir
                    .join(format!("page/{}/index.html", page_num))
            };

            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, html)?;
            tracing::debug!("Generated: {:?}", output_path);
        }

        Ok(())
    }

    /// Generate individual post pages
    fn generate_post_pages(
        &self,
        posts: &[Post],
        site_data: &SiteData,
        config_data: &ConfigData,
        theme_data: &ThemeData,
    ) -> Result<()> {
        let all_posts: Vec<_> = posts.to_vec();

        for (i, post) in posts.iter().enumerate() {
            // Compute prev/next navigation
            let prev_post = if i + 1 < all_posts.len() {
                Some(NavPost {
                    title: all_posts[i + 1].title.clone(),
                    path: format!("/{}", all_posts[i + 1].path.trim_start_matches('/')),
                })
            } else {
                None
            };

            let next_post = if i > 0 {
                Some(NavPost {
                    title: all_posts[i - 1].title.clone(),
                    path: format!("/{}", all_posts[i - 1].path.trim_start_matches('/')),
                })
            } else {
                None
            };

            // Generate table of contents
            let toc_html = toc(&post.content, 3);
            // Check if TOC has actual content (not just empty <ol class="toc"></ol>)
            let has_toc = toc_html.contains("toc-item");

            let mut context = self.create_base_context(site_data, config_data, theme_data);
            context.insert("page_title", &post.title);
            context.insert("page_date", &post.date.format("%Y-%m-%d").to_string());
            context.insert("page_content", &post.content);
            context.insert("page_tags", &post.tags);
            context.insert("page_categories", &post.categories);
            context.insert("page_banner", &"");
            context.insert("page_mathjax", &false);
            context.insert("current_path", &post.path);
            // Only show catalog if theme enables it AND there's actual TOC content
            context.insert("show_catalog", &(theme_data.catalog && has_toc));
            context.insert("is_special_page", &false);
            context.insert("toc", &toc_html);

            if let Some(ref prev) = prev_post {
                context.insert("prev_post", prev);
            }
            if let Some(ref next) = next_post {
                context.insert("next_post", next);
            }

            let html = self.renderer.render("page.html", &context)?;

            // Strip leading slash from path to avoid creating absolute paths
            let clean_path = post.path.trim_start_matches('/');
            let output_path = self.hexo.public_dir.join(clean_path).join("index.html");
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("Failed to create dir {:?}: {}", parent, e))?;
            }
            fs::write(&output_path, &html)
                .map_err(|e| anyhow::anyhow!("Failed to write {:?}: {}", output_path, e))?;
            tracing::debug!("Generated post: {:?}", output_path);
        }

        Ok(())
    }

    /// Generate standalone pages
    fn generate_page_pages(
        &self,
        pages: &[Page],
        site_data: &SiteData,
        config_data: &ConfigData,
        theme_data: &ThemeData,
    ) -> Result<()> {
        for page in pages {
            let template_name = match page.layout.as_str() {
                "about" => "about.html",
                "links" => "links.html",
                "project" => "project.html",
                "search" => "search.html",
                "home" => "home.html",
                "tags" => "tags.html",
                _ => "page.html",
            };

            let mut context = self.create_base_context(site_data, config_data, theme_data);
            context.insert("page_title", &page.title);
            context.insert("page_date", &page.date.format("%Y-%m-%d").to_string());
            context.insert("page_content", &page.content);
            context.insert("page_tags", &Vec::<String>::new());
            context.insert("page_banner", &"");
            context.insert("page_mathjax", &false);
            context.insert("current_path", &page.path);
            context.insert("show_catalog", &false);
            context.insert("is_special_page", &true);

            // Special handling for tags page - provide all_tags data
            if page.layout == "tags" {
                let all_tags = self.build_all_tags_data(site_data);
                context.insert("all_tags", &all_tags);
            }

            let html = self.renderer.render(template_name, &context)?;

            // Strip leading slash from path to avoid creating absolute paths
            let clean_path = page.path.trim_start_matches('/');
            let output_path = self.hexo.public_dir.join(clean_path).join("index.html");
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, html)?;
            tracing::debug!("Generated page: {:?}", output_path);
        }

        Ok(())
    }

    /// Build all tags data for the tags listing page
    fn build_all_tags_data(&self, site_data: &SiteData) -> Vec<TagData> {
        // Group posts by tag
        let mut tags_map: HashMap<String, Vec<PostData>> = HashMap::new();

        for post in &site_data.posts {
            for tag in &post.tags {
                // Skip empty tags
                if tag.trim().is_empty() {
                    continue;
                }
                tags_map.entry(tag.clone()).or_default().push(PostData {
                    title: post.title.clone(),
                    date: post.date.clone(),
                    path: post.path.clone(),
                    permalink: post.permalink.clone(),
                    tags: post.tags.clone(),
                    categories: post.categories.clone(),
                    content: String::new(), // Don't need content for listing
                    excerpt: None,
                    word_count: 0,
                });
            }
        }

        // Convert to sorted vector
        let mut all_tags: Vec<TagData> = tags_map
            .into_iter()
            .map(|(name, posts)| TagData { name, posts })
            .collect();

        // Sort by tag name
        all_tags.sort_by(|a, b| a.name.cmp(&b.name));

        all_tags
    }

    /// Generate archive page
    fn generate_archive_page(
        &self,
        posts: &[Post],
        site_data: &SiteData,
        config_data: &ConfigData,
        theme_data: &ThemeData,
    ) -> Result<()> {
        // Group posts by year
        let mut years_map: BTreeMap<i32, Vec<PostData>> = BTreeMap::new();

        for post in posts {
            let year = post.date.year();
            years_map.entry(year).or_default().push(PostData {
                title: post.title.clone(),
                date: post.date.format("%Y-%m-%d").to_string(),
                path: format!("/{}", post.path.trim_start_matches('/')),
                permalink: post.permalink.clone(),
                tags: post.tags.clone(),
                categories: post.categories.clone(),
                content: String::new(), // Don't need full content for archive
                excerpt: None,
                word_count: 0,
            });
        }

        // Convert to sorted vector (newest first)
        let archive_years: Vec<ArchiveYearData> = years_map
            .into_iter()
            .rev()
            .map(|(year, posts)| ArchiveYearData { year, posts })
            .collect();

        let mut context = self.create_base_context(site_data, config_data, theme_data);
        context.insert("archive_years", &archive_years);
        context.insert("current_path", "archives/");
        context.insert("is_home", &false);

        let html = self.renderer.render("archive.html", &context)?;

        let output_path = self
            .hexo
            .public_dir
            .join(&self.hexo.config.archive_dir)
            .join("index.html");
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, html)?;
        tracing::info!("Generated archive page");

        Ok(())
    }

    /// Generate tag pages
    fn generate_tag_pages(
        &self,
        posts: &[Post],
        site_data: &SiteData,
        config_data: &ConfigData,
        theme_data: &ThemeData,
    ) -> Result<()> {
        // Group posts by tag
        let mut tags_map: HashMap<String, Vec<PostData>> = HashMap::new();

        for post in posts {
            for tag in &post.tags {
                // Skip empty tags
                if tag.trim().is_empty() {
                    continue;
                }
                tags_map.entry(tag.clone()).or_default().push(PostData {
                    title: post.title.clone(),
                    date: post.date.format("%Y-%m-%d").to_string(),
                    path: format!("/{}", post.path.trim_start_matches('/')),
                    permalink: post.permalink.clone(),
                    tags: post.tags.clone(),
                    categories: post.categories.clone(),
                    content: String::new(),
                    excerpt: None,
                    word_count: 0,
                });
            }
        }

        // Generate individual tag pages
        for (tag, tag_posts) in &tags_map {
            // Skip empty tags
            if tag.trim().is_empty() {
                continue;
            }

            let tag_slug = slug::slugify(tag);

            // Skip if slug is empty (shouldn't happen but be safe)
            if tag_slug.is_empty() {
                continue;
            }

            let mut context = self.create_base_context(site_data, config_data, theme_data);
            context.insert("tag_name", tag);
            context.insert("tag_posts", tag_posts);
            context.insert("current_path", &format!("tags/{}/", tag_slug));
            context.insert("is_home", &false);

            let html = self.renderer.render("tag_single.html", &context)?;

            let output_path = self
                .hexo
                .public_dir
                .join(&self.hexo.config.tag_dir)
                .join(&tag_slug)
                .join("index.html");
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, html)?;
        }

        tracing::info!("Generated {} tag pages", tags_map.len());
        Ok(())
    }

    /// Generate Atom RSS feed
    fn generate_atom_feed(&self, posts: &[Post]) -> Result<()> {
        let mut feed = String::new();
        feed.push_str(r#"<?xml version="1.0" encoding="utf-8"?>"#);
        feed.push('\n');
        feed.push_str(r#"<feed xmlns="http://www.w3.org/2005/Atom">"#);
        feed.push('\n');
        feed.push_str(&format!(
            "  <title>{}</title>\n",
            escape_xml(&self.hexo.config.title)
        ));
        feed.push_str(&format!(
            "  <link href=\"{}/atom.xml\" rel=\"self\"/>\n",
            self.hexo.config.url
        ));
        feed.push_str(&format!("  <link href=\"{}/\"/>\n", self.hexo.config.url));
        feed.push_str(&format!(
            "  <updated>{}</updated>\n",
            chrono::Utc::now().to_rfc3339()
        ));
        feed.push_str(&format!("  <id>{}/</id>\n", self.hexo.config.url));
        feed.push_str(&format!(
            "  <author><name>{}</name></author>\n",
            escape_xml(&self.hexo.config.author)
        ));

        // Include recent posts (limit to 20)
        for post in posts.iter().take(20) {
            feed.push_str("  <entry>\n");
            feed.push_str(&format!("    <title>{}</title>\n", escape_xml(&post.title)));
            feed.push_str(&format!(
                "    <link href=\"{}{}\"/>\n",
                self.hexo.config.url.trim_end_matches('/'),
                if post.path.starts_with('/') {
                    post.path.clone()
                } else {
                    format!("/{}", post.path)
                }
            ));
            feed.push_str(&format!(
                "    <id>{}{}</id>\n",
                self.hexo.config.url.trim_end_matches('/'),
                if post.path.starts_with('/') {
                    post.path.clone()
                } else {
                    format!("/{}", post.path)
                }
            ));
            feed.push_str(&format!(
                "    <published>{}</published>\n",
                post.date.to_rfc3339()
            ));
            feed.push_str(&format!(
                "    <updated>{}</updated>\n",
                post.updated.unwrap_or(post.date).to_rfc3339()
            ));
            // Convert relative URLs in content to absolute URLs
            let content = post.excerpt.as_ref().unwrap_or(&post.content);
            let base_url = self.hexo.config.url.trim_end_matches('/');
            let content_with_full_urls = convert_relative_urls_to_absolute(content, base_url);
            // Strip invalid XML control characters
            let clean_content = strip_invalid_xml_chars(&content_with_full_urls);
            feed.push_str(&format!(
                "    <content type=\"html\"><![CDATA[{}]]></content>\n",
                clean_content
            ));
            feed.push_str("  </entry>\n");
        }

        feed.push_str("</feed>\n");

        let output_path = self.hexo.public_dir.join("atom.xml");
        fs::write(&output_path, feed)?;
        tracing::info!("Generated atom.xml");

        Ok(())
    }

    /// Generate search index (JSON)
    fn generate_search_index(&self, posts: &[Post]) -> Result<()> {
        let search_data: Vec<serde_json::Value> = posts
            .iter()
            .map(|p| {
                serde_json::json!({
                    "title": p.title,
                    "url": format!("/{}", p.path.trim_start_matches('/')),
                    "content": strip_html(&p.content),
                    "date": p.date.format("%Y-%m-%d").to_string(),
                })
            })
            .collect();

        let output_path = self.hexo.public_dir.join("search.json");
        let json = serde_json::to_string_pretty(&search_data)?;
        fs::write(&output_path, json)?;
        tracing::info!("Generated search.json");

        Ok(())
    }

    /// Copy source assets (images, etc.) to public directory
    fn copy_source_assets(&self) -> Result<()> {
        let source_dir = &self.hexo.source_dir;

        for entry in WalkDir::new(source_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());

                // Skip markdown files (they are processed separately)
                if matches!(ext, Some("md") | Some("markdown")) {
                    continue;
                }

                // Skip files in _posts directory
                if path
                    .components()
                    .any(|c| c.as_os_str() == "_posts" || c.as_os_str() == "_drafts")
                {
                    continue;
                }

                let relative = path.strip_prefix(source_dir)?;
                let dest = self.hexo.public_dir.join(relative);

                if let Some(parent) = dest.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::copy(path, &dest)?;
            }
        }

        Ok(())
    }
}

/// Count words in HTML content (strips tags first)
fn count_words(html: &str) -> usize {
    let text = strip_html(html);
    // Count Chinese characters and English words
    let mut count = 0;
    let mut in_word = false;

    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if !in_word {
                in_word = true;
                count += 1;
            }
        } else if c > '\u{4E00}' && c < '\u{9FFF}' {
            // Chinese characters
            count += 1;
            in_word = false;
        } else {
            in_word = false;
        }
    }

    count
}

/// Strip HTML tags from content
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

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// Import chrono Datelike trait for year()
use chrono::Datelike;
use chrono::Timelike;

/// Format datetime with Chinese AM/PM (e.g., "2026-01-31, 上午 11:02")
fn format_datetime_chinese<Tz: chrono::TimeZone>(dt: &chrono::DateTime<Tz>) -> String
where
    <Tz as chrono::TimeZone>::Offset: std::fmt::Display,
{
    let hour = dt.hour();
    let (period, hour_12) = if hour < 12 {
        ("上午", if hour == 0 { 12 } else { hour })
    } else {
        ("下午", if hour == 12 { 12 } else { hour - 12 })
    };
    format!(
        "{}, {} {:02}:{:02}",
        dt.format("%Y-%m-%d"),
        period,
        hour_12,
        dt.minute()
    )
}

/// Convert relative URLs in HTML content to absolute URLs
/// Handles href="/...", src="/...", and similar patterns
fn convert_relative_urls_to_absolute(content: &str, base_url: &str) -> String {
    // Replace href="/ and src="/ with absolute URLs
    let result = content
        .replace("href=\"/", &format!("href=\"{}/", base_url))
        .replace("src=\"/", &format!("src=\"{}/", base_url))
        .replace("href='/", &format!("href='{}/", base_url))
        .replace("src='/", &format!("src='{}/", base_url));
    result
}

/// Strip invalid XML control characters (except tab, newline, carriage return)
/// XML 1.0 only allows: #x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] | [#x10000-#x10FFFF]
fn strip_invalid_xml_chars(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            c == '\t'
                || c == '\n'
                || c == '\r'
                || ('\u{0020}'..='\u{D7FF}').contains(&c)
                || ('\u{E000}'..='\u{FFFD}').contains(&c)
                || ('\u{10000}'..='\u{10FFFF}').contains(&c)
        })
        .collect()
}
