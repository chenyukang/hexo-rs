//! Create a new post or page

use anyhow::Result;
use std::fs;

use crate::Hexo;

/// Create a new post/page/draft
pub fn create_post(hexo: &Hexo, title: &str, layout: &str, path: Option<&str>) -> Result<()> {
    let now = chrono::Local::now();

    // Determine the target directory based on layout
    let target_dir = match layout {
        "draft" => hexo.source_dir.join("_drafts"),
        "page" => {
            let slug = slug::slugify(title);
            hexo.source_dir.join(&slug)
        }
        _ => hexo.source_dir.join("_posts"),
    };

    fs::create_dir_all(&target_dir)?;

    // Generate filename
    let filename = if let Some(p) = path {
        format!("{}.md", p)
    } else {
        let post_name = &hexo.config.new_post_name;
        let slug = slug::slugify(title);

        post_name
            .replace(":title", &slug)
            .replace(":year", &now.format("%Y").to_string())
            .replace(":month", &now.format("%m").to_string())
            .replace(":day", &now.format("%d").to_string())
            .replace(":i_month", &now.format("%-m").to_string())
            .replace(":i_day", &now.format("%-d").to_string())
    };

    let file_path = if layout == "page" {
        target_dir.join("index.md")
    } else {
        target_dir.join(&filename)
    };

    // Load scaffold template
    let scaffold_path = hexo
        .base_dir
        .join("scaffolds")
        .join(format!("{}.md", layout));
    let scaffold_content = if scaffold_path.exists() {
        fs::read_to_string(&scaffold_path)?
    } else {
        // Default scaffold
        format!(
            r#"---
title: {{{{ title }}}}
date: {{{{ date }}}}
---
"#
        )
    };

    // Replace template variables
    let content = scaffold_content
        .replace("{{ title }}", title)
        .replace("{{ date }}", &now.format("%Y-%m-%d %H:%M:%S").to_string());

    // Check if file already exists
    if file_path.exists() {
        anyhow::bail!("File already exists: {:?}", file_path);
    }

    fs::write(&file_path, content)?;

    println!("Created: {:?}", file_path);

    Ok(())
}

/// Run the new command
pub fn run(hexo: &Hexo, title: &str, layout: Option<&str>) -> Result<()> {
    let layout = layout.unwrap_or(&hexo.config.default_layout);
    create_post(hexo, title, layout, None)
}
