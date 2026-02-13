//! Theme loader - loads theme configuration and assets
//!
//! Note: Template rendering is now handled by the templates module using Tera.
//! This module is responsible for:
//! - Loading theme configuration from _config.yml
//! - Copying theme assets (CSS, JS, images) to the public directory

use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Theme loader - loads config and copies assets
pub struct ThemeLoader {
    /// Theme directory path
    theme_dir: PathBuf,
    /// Theme configuration (IndexMap preserves YAML key order for menu items)
    config: IndexMap<String, serde_yaml::Value>,
}

impl ThemeLoader {
    /// Load a theme from a directory
    pub fn load<P: AsRef<Path>>(theme_dir: P) -> Result<Self> {
        let theme_dir = theme_dir.as_ref().to_path_buf();

        if !theme_dir.exists() {
            return Err(anyhow!("Theme directory not found: {:?}", theme_dir));
        }

        let mut loader = Self {
            theme_dir: theme_dir.clone(),
            config: IndexMap::new(),
        };

        // Load theme config
        let config_path = theme_dir.join("_config.yml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            loader.config = serde_yaml::from_str(&content)?;
        }

        Ok(loader)
    }

    /// Get theme configuration
    pub fn config(&self) -> &IndexMap<String, serde_yaml::Value> {
        &self.config
    }

    /// Copy theme source files to public directory
    pub fn copy_source(&self, public_dir: &Path) -> Result<()> {
        let source_dir = self.theme_dir.join("source");
        if !source_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(&source_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() {
                let relative = path.strip_prefix(&source_dir)?;

                // Skip files in directories starting with _ or . (e.g., _partial/)
                let should_skip = relative.components().any(|c| {
                    c.as_os_str()
                        .to_str()
                        .map(|s| s.starts_with('_') || s.starts_with('.'))
                        .unwrap_or(false)
                });
                if should_skip {
                    continue;
                }

                let ext = path.extension().and_then(|e| e.to_str());

                // Handle Stylus files - compile to CSS
                if ext == Some("styl") {
                    // Check if there's a pre-compiled CSS file
                    let css_path = path.with_extension("css");
                    if css_path.exists() {
                        // Use pre-compiled CSS
                        let css_relative = relative.with_extension("css");
                        let dest = public_dir.join(&css_relative);
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::copy(&css_path, &dest)?;
                        tracing::debug!("Copied pre-compiled CSS: {:?} -> {:?}", css_path, dest);
                    } else {
                        // Try to compile with npx stylus
                        let css_relative = relative.with_extension("css");
                        let dest = public_dir.join(&css_relative);

                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent)?;
                        }

                        match compile_stylus(path, &source_dir) {
                            Ok(css) => {
                                fs::write(&dest, css)?;
                                tracing::info!("Compiled Stylus: {:?} -> {:?}", path, dest);
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to compile {:?}: {}. \
                                    Please run 'npx stylus {} -o {}' to pre-compile, \
                                    or place a pre-compiled style.css alongside the .styl file.",
                                    path,
                                    e,
                                    path.display(),
                                    path.parent().unwrap_or(Path::new(".")).display()
                                );
                                return Err(anyhow!(
                                    "Stylus compilation failed for {:?}. \
                                    Install stylus (npm install -g stylus) or provide pre-compiled CSS.",
                                    path
                                ));
                            }
                        }
                    }
                } else {
                    let dest = public_dir.join(relative);

                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    fs::copy(path, &dest)?;
                    tracing::debug!("Copied: {:?} -> {:?}", path, dest);
                }
            }
        }

        Ok(())
    }
}

/// Compile a Stylus file to CSS using npx stylus
fn compile_stylus(styl_path: &Path, include_dir: &Path) -> Result<String> {
    use std::process::Command;

    // Try npx stylus first
    let output = Command::new("npx")
        .args([
            "stylus",
            "--print",
            "--include",
            include_dir.to_str().unwrap_or("."),
            styl_path.to_str().unwrap_or(""),
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            Err(anyhow!("Stylus compilation failed: {}", stderr))
        }
        Err(e) => Err(anyhow!("Failed to run npx stylus: {}", e)),
    }
}
