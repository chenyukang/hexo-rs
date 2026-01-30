//! Clean the public directory

use anyhow::Result;
use std::fs;

use crate::Hexo;

/// Clean the public directory and cache
pub fn run(hexo: &Hexo) -> Result<()> {
    if hexo.public_dir.exists() {
        fs::remove_dir_all(&hexo.public_dir)?;
        tracing::info!("Deleted: {:?}", hexo.public_dir);
    }

    // Also clean the database cache if it exists
    let db_path = hexo.base_dir.join("db.json");
    if db_path.exists() {
        fs::remove_file(&db_path)?;
        tracing::info!("Deleted: {:?}", db_path);
    }

    Ok(())
}
