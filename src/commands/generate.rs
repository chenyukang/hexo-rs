//! Generate static files

use anyhow::Result;
use notify::Watcher;
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::content::loader::ContentLoader;
use crate::generator::Generator;
use crate::Hexo;

/// Generate the static site
pub fn run(hexo: &Hexo) -> Result<()> {
    let start = std::time::Instant::now();

    // Load content
    let loader = ContentLoader::new(hexo);
    let posts = loader.load_posts()?;
    let pages = loader.load_pages()?;

    tracing::info!("Loaded {} posts and {} pages", posts.len(), pages.len());

    // Generate site
    let generator = Generator::new(hexo)?;
    generator.generate(&posts, &pages)?;

    let duration = start.elapsed();
    tracing::info!("Generated in {:.2}s", duration.as_secs_f64());

    Ok(())
}

/// Watch for file changes and regenerate
pub async fn watch(hexo: &Hexo) -> Result<()> {
    let (tx, rx) = channel();

    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;

    // Watch source directory
    watcher.watch(
        hexo.source_dir.as_ref(),
        notify::RecursiveMode::Recursive,
    )?;

    // Watch theme directory
    if hexo.theme_dir.exists() {
        watcher.watch(
            hexo.theme_dir.as_ref(),
            notify::RecursiveMode::Recursive,
        )?;
    }

    // Watch config file
    watcher.watch(
        Path::new(&hexo.base_dir.join("_config.yml")),
        notify::RecursiveMode::NonRecursive,
    )?;

    tracing::info!("Watching for changes. Press Ctrl+C to stop.");

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
