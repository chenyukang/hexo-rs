//! Generate static files

use anyhow::Result;
use notify::Watcher;
use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::cache::{self, CacheDb, ChangeSet};
use crate::content::loader::ContentLoader;
use crate::generator::Generator;
use crate::Hexo;

/// Generate the static site (with incremental support)
pub fn run(hexo: &Hexo) -> Result<()> {
    run_with_options(hexo, false)
}

/// Generate with force option
pub fn run_with_options(hexo: &Hexo, force: bool) -> Result<()> {
    let start = std::time::Instant::now();

    // Load content
    let loader = ContentLoader::new(hexo);
    let posts = loader.load_posts()?;
    let pages = loader.load_pages()?;

    tracing::info!("Loaded {} posts and {} pages", posts.len(), pages.len());

    // Load cache
    let cache = CacheDb::load(&hexo.base_dir);

    // Calculate current file hashes
    let current_posts: Vec<_> = posts
        .iter()
        .map(|p| {
            let hash = cache::hash_content(&p.raw);
            (p.source.clone(), hash, p.tags.clone(), p.categories.clone())
        })
        .collect();

    let current_pages: Vec<_> = pages
        .iter()
        .map(|p| {
            let hash = cache::hash_content(&p.raw);
            (p.source.clone(), hash)
        })
        .collect();

    // Detect changes
    let changeset = if force || cache.post_count == 0 {
        tracing::info!(
            "Full generation (force={}, cache_empty={})",
            force,
            cache.post_count == 0
        );
        ChangeSet::full_rebuild()
    } else {
        cache::detect_changes(
            &cache,
            &hexo.base_dir,
            &hexo.source_dir,
            &hexo.theme_dir,
            &current_posts,
            &current_pages,
        )?
    };

    // Generate site
    let generator = Generator::new(hexo)?;

    if !changeset.has_changes() {
        tracing::info!("No changes detected, skipping generation");
        let duration = start.elapsed();
        tracing::info!("Completed in {:.2}s (no changes)", duration.as_secs_f64());
        return Ok(());
    }

    tracing::info!("Changes detected: {}", changeset.summary());

    if changeset.full_rebuild {
        generator.generate(&posts, &pages)?;
    } else {
        generator.generate_incremental(&posts, &pages, &changeset)?;
    }

    // Update cache
    let mut new_cache = CacheDb::new();
    let posts_for_cache: Vec<_> = posts
        .iter()
        .map(|p| {
            let hash = cache::hash_content(&p.raw);
            (
                p.source.clone(),
                hash,
                p.path.clone(),
                p.tags.clone(),
                p.categories.clone(),
            )
        })
        .collect();

    let pages_for_cache: Vec<_> = pages
        .iter()
        .map(|p| {
            let hash = cache::hash_content(&p.raw);
            (p.source.clone(), hash, p.path.clone())
        })
        .collect();

    cache::update_cache(
        &mut new_cache,
        &hexo.base_dir,
        &hexo.theme_dir,
        &posts_for_cache,
        &pages_for_cache,
    )?;

    new_cache.save(&hexo.base_dir)?;

    let duration = start.elapsed();
    tracing::info!("Generated in {:.2}s", duration.as_secs_f64());

    Ok(())
}

/// Watch for file changes and regenerate (with incremental support)
pub async fn watch(hexo: &Hexo) -> Result<()> {
    let (tx, rx) = channel();

    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;

    // Watch source directory
    watcher.watch(hexo.source_dir.as_ref(), notify::RecursiveMode::Recursive)?;

    // Watch theme directory
    if hexo.theme_dir.exists() {
        watcher.watch(hexo.theme_dir.as_ref(), notify::RecursiveMode::Recursive)?;
    }

    // Watch config file
    watcher.watch(
        Path::new(&hexo.base_dir.join("_config.yml")),
        notify::RecursiveMode::NonRecursive,
    )?;

    tracing::info!("Watching for changes (incremental mode). Press Ctrl+C to stop.");

    // Debounce events
    let mut last_rebuild = std::time::Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(_event) => {
                // Debounce: only rebuild if more than 500ms since last rebuild
                if last_rebuild.elapsed() > Duration::from_millis(500) {
                    tracing::info!("File changed, regenerating...");
                    if let Err(e) = run(hexo) {
                        tracing::error!("Generation failed: {}", e);
                    }
                    last_rebuild = std::time::Instant::now();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Continue waiting
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

/// Clear the cache
pub fn clear_cache(hexo: &Hexo) -> Result<()> {
    let cache_dir = hexo.base_dir.join(".hexo-cache");
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
        tracing::info!("Cache cleared");
    }
    Ok(())
}
