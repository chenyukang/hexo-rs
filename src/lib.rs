//! hexo-rs: A fast static site generator with built-in vexo theme
//!
//! This crate provides a Rust implementation of a static site generator
//! that uses Tera templates with an embedded vexo theme for rendering.

pub mod commands;
pub mod config;
pub mod content;
pub mod generator;
pub mod helpers;
pub mod server;
pub mod templates;
pub mod theme;

use anyhow::Result;
use std::path::Path;

/// The main Hexo application
#[derive(Clone)]
pub struct Hexo {
    /// Site configuration
    pub config: config::SiteConfig,
    /// Base directory
    pub base_dir: std::path::PathBuf,
    /// Source directory
    pub source_dir: std::path::PathBuf,
    /// Public (output) directory
    pub public_dir: std::path::PathBuf,
    /// Theme directory
    pub theme_dir: std::path::PathBuf,
}

impl Hexo {
    /// Create a new Hexo instance from a directory
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        let config_path = base_dir.join("_config.yml");

        let config = if config_path.exists() {
            config::SiteConfig::load(&config_path)?
        } else {
            config::SiteConfig::default()
        };

        let source_dir = base_dir.join(&config.source_dir);
        let public_dir = base_dir.join(&config.public_dir);
        let theme_dir = base_dir.join("themes").join(&config.theme);

        Ok(Self {
            config,
            base_dir,
            source_dir,
            public_dir,
            theme_dir,
        })
    }

    /// Initialize a new site
    pub fn init(&self) -> Result<()> {
        commands::init::run(self)
    }

    /// Generate the static site
    pub fn generate(&self) -> Result<()> {
        commands::generate::run(self)
    }

    /// Clean the public directory
    pub fn clean(&self) -> Result<()> {
        commands::clean::run(self)
    }

    /// Create a new post
    pub fn new_post(&self, title: &str, layout: Option<&str>) -> Result<()> {
        commands::new::run(self, title, layout)
    }
}
