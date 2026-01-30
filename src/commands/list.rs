//! List site content

use anyhow::Result;

use crate::content::loader::ContentLoader;
use crate::Hexo;

/// List site content by type
pub fn run(hexo: &Hexo, content_type: &str) -> Result<()> {
    let loader = ContentLoader::new(hexo);

    match content_type {
        "post" | "posts" => {
            let posts = loader.load_posts()?;
            println!("Posts ({}):", posts.len());
            for post in posts {
                println!(
                    "  {} - {} [{}]",
                    post.date.format("%Y-%m-%d"),
                    post.title,
                    post.source
                );
            }
        }
        "page" | "pages" => {
            let pages = loader.load_pages()?;
            println!("Pages ({}):", pages.len());
            for page in pages {
                println!("  {} [{}]", page.title, page.source);
            }
        }
        "tag" | "tags" => {
            let posts = loader.load_posts()?;
            let mut tags: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for post in &posts {
                for tag in &post.tags {
                    *tags.entry(tag.clone()).or_insert(0) += 1;
                }
            }
            println!("Tags ({}):", tags.len());
            let mut tags: Vec<_> = tags.into_iter().collect();
            tags.sort_by(|a, b| b.1.cmp(&a.1));
            for (tag, count) in tags {
                println!("  {} ({})", tag, count);
            }
        }
        "category" | "categories" => {
            let posts = loader.load_posts()?;
            let mut categories: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for post in &posts {
                for cat in &post.categories {
                    *categories.entry(cat.clone()).or_insert(0) += 1;
                }
            }
            println!("Categories ({}):", categories.len());
            let mut categories: Vec<_> = categories.into_iter().collect();
            categories.sort_by(|a, b| b.1.cmp(&a.1));
            for (cat, count) in categories {
                println!("  {} ({})", cat, count);
            }
        }
        _ => {
            anyhow::bail!(
                "Unknown type: {}. Available: post, page, tag, category",
                content_type
            );
        }
    }

    Ok(())
}
