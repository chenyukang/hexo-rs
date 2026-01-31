//! Cache module for incremental generation
//!
//! This module provides caching functionality to enable incremental site generation.
//! It tracks file hashes and modification times to detect changes and avoid
//! regenerating unchanged content.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Cache file name
const CACHE_FILE: &str = ".hexo-cache/db.json";

/// Represents a cached entry for a source file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Content hash of the source file
    pub content_hash: u64,
    /// Last modification time (as unix timestamp)
    pub mtime: u64,
    /// Output path relative to public dir
    pub output_path: String,
    /// Tags associated with this post (for detecting tag page updates)
    pub tags: Vec<String>,
    /// Categories associated with this post
    pub categories: Vec<String>,
}

/// Cache database for tracking file changes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheDb {
    /// Version of the cache format
    pub version: u32,
    /// Hash of the theme directory (changes trigger full rebuild)
    pub theme_hash: u64,
    /// Hash of the site config (changes trigger full rebuild)
    pub config_hash: u64,
    /// Cached entries for posts, keyed by source path
    pub posts: HashMap<String, CacheEntry>,
    /// Cached entries for pages, keyed by source path
    pub pages: HashMap<String, CacheEntry>,
    /// Hash of all tags (for detecting if tag pages need rebuild)
    pub tags_hash: u64,
    /// Hash of all categories
    pub categories_hash: u64,
    /// Total post count (for detecting additions/deletions)
    pub post_count: usize,
}

impl CacheDb {
    /// Current cache format version
    const VERSION: u32 = 1;

    /// Load cache from disk, or create a new empty cache
    pub fn load(base_dir: &Path) -> Self {
        let cache_path = base_dir.join(CACHE_FILE);
        if let Ok(content) = fs::read_to_string(&cache_path) {
            if let Ok(cache) = serde_json::from_str::<CacheDb>(&content) {
                if cache.version == Self::VERSION {
                    return cache;
                }
                tracing::info!("Cache version mismatch, rebuilding cache");
            }
        }
        Self::default()
    }

    /// Save cache to disk
    pub fn save(&self, base_dir: &Path) -> Result<()> {
        let cache_dir = base_dir.join(".hexo-cache");
        fs::create_dir_all(&cache_dir)?;

        let cache_path = base_dir.join(CACHE_FILE);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(cache_path, content)?;
        Ok(())
    }

    /// Create a new cache with version set
    pub fn new() -> Self {
        Self {
            version: Self::VERSION,
            ..Default::default()
        }
    }
}

/// Change detection result
#[derive(Debug, Clone)]
pub struct ChangeSet {
    /// Posts that need regeneration (source path)
    pub changed_posts: Vec<String>,
    /// Pages that need regeneration (source path)
    pub changed_pages: Vec<String>,
    /// Posts that were deleted
    pub deleted_posts: Vec<String>,
    /// Pages that were deleted
    pub deleted_pages: Vec<String>,
    /// Whether index pages need regeneration
    pub rebuild_index: bool,
    /// Whether archive pages need regeneration
    pub rebuild_archives: bool,
    /// Whether tag pages need regeneration (specific tags or all)
    pub rebuild_tags: RebuildScope,
    /// Whether category pages need regeneration
    pub rebuild_categories: RebuildScope,
    /// Whether to regenerate everything (theme/config changed)
    pub full_rebuild: bool,
}

/// Scope of rebuild for tags/categories
#[derive(Debug, Clone)]
pub enum RebuildScope {
    /// No rebuild needed
    None,
    /// Only rebuild specific items
    Specific(Vec<String>),
    /// Rebuild all
    All,
}

impl ChangeSet {
    /// Create a changeset indicating full rebuild is needed
    pub fn full_rebuild() -> Self {
        Self {
            changed_posts: Vec::new(),
            changed_pages: Vec::new(),
            deleted_posts: Vec::new(),
            deleted_pages: Vec::new(),
            rebuild_index: true,
            rebuild_archives: true,
            rebuild_tags: RebuildScope::All,
            rebuild_categories: RebuildScope::All,
            full_rebuild: true,
        }
    }

    /// Create an empty changeset (no changes)
    pub fn empty() -> Self {
        Self {
            changed_posts: Vec::new(),
            changed_pages: Vec::new(),
            deleted_posts: Vec::new(),
            deleted_pages: Vec::new(),
            rebuild_index: false,
            rebuild_archives: false,
            rebuild_tags: RebuildScope::None,
            rebuild_categories: RebuildScope::None,
            full_rebuild: false,
        }
    }

    /// Check if any changes were detected
    pub fn has_changes(&self) -> bool {
        self.full_rebuild
            || !self.changed_posts.is_empty()
            || !self.changed_pages.is_empty()
            || !self.deleted_posts.is_empty()
            || !self.deleted_pages.is_empty()
            || self.rebuild_index
            || self.rebuild_archives
            || !matches!(self.rebuild_tags, RebuildScope::None)
            || !matches!(self.rebuild_categories, RebuildScope::None)
    }

    /// Get summary of changes for logging
    pub fn summary(&self) -> String {
        if self.full_rebuild {
            return "full rebuild required".to_string();
        }

        let mut parts = Vec::new();
        if !self.changed_posts.is_empty() {
            parts.push(format!("{} posts changed", self.changed_posts.len()));
        }
        if !self.changed_pages.is_empty() {
            parts.push(format!("{} pages changed", self.changed_pages.len()));
        }
        if !self.deleted_posts.is_empty() {
            parts.push(format!("{} posts deleted", self.deleted_posts.len()));
        }
        if self.rebuild_index {
            parts.push("index pages".to_string());
        }
        if self.rebuild_archives {
            parts.push("archive pages".to_string());
        }
        match &self.rebuild_tags {
            RebuildScope::None => {}
            RebuildScope::Specific(tags) => parts.push(format!("{} tag pages", tags.len())),
            RebuildScope::All => parts.push("all tag pages".to_string()),
        }
        match &self.rebuild_categories {
            RebuildScope::None => {}
            RebuildScope::Specific(cats) => parts.push(format!("{} category pages", cats.len())),
            RebuildScope::All => parts.push("all category pages".to_string()),
        }

        if parts.is_empty() {
            "no changes".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Calculate a hash for file content
pub fn hash_content(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Calculate a hash for a file on disk
pub fn hash_file(path: &Path) -> Result<u64> {
    let content = fs::read_to_string(path)?;
    Ok(hash_content(&content))
}

/// Get file modification time as unix timestamp
pub fn get_mtime(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path)?;
    let mtime = metadata.modified()?;
    Ok(mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs())
}

/// Calculate hash for a directory (for theme change detection)
pub fn hash_directory(dir: &Path) -> Result<u64> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use walkdir::WalkDir;

    let mut hasher = DefaultHasher::new();

    // Collect and sort paths for deterministic ordering
    let mut paths: Vec<_> = WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    paths.sort();

    for path in paths {
        // Only hash template and config files, not assets
        let ext = path.extension().and_then(|e| e.to_str());
        if matches!(ext, Some("ejs") | Some("yml") | Some("yaml") | Some("json")) {
            if let Ok(content) = fs::read_to_string(&path) {
                path.to_string_lossy().hash(&mut hasher);
                content.hash(&mut hasher);
            }
        }
    }

    Ok(hasher.finish())
}

/// Calculate hash for site config
pub fn hash_config(config_path: &Path) -> Result<u64> {
    hash_file(config_path)
}

/// Calculate hash for a set of strings (tags or categories)
pub fn hash_string_set(items: &[String]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    let mut sorted: Vec<_> = items.to_vec();
    sorted.sort();
    sorted.hash(&mut hasher);
    hasher.finish()
}

/// Detect changes between current state and cached state
pub fn detect_changes(
    cache: &CacheDb,
    base_dir: &Path,
    _source_dir: &Path,
    theme_dir: &Path,
    current_posts: &[(String, u64, Vec<String>, Vec<String>)], // (source, hash, tags, categories)
    current_pages: &[(String, u64)],                           // (source, hash)
) -> Result<ChangeSet> {
    // Check theme changes
    let theme_hash = if theme_dir.exists() {
        hash_directory(theme_dir)?
    } else {
        0
    };

    if theme_hash != cache.theme_hash && cache.theme_hash != 0 {
        tracing::info!("Theme changed, full rebuild required");
        return Ok(ChangeSet::full_rebuild());
    }

    // Check config changes
    let config_path = base_dir.join("_config.yml");
    let config_hash = if config_path.exists() {
        hash_file(&config_path)?
    } else {
        0
    };

    if config_hash != cache.config_hash && cache.config_hash != 0 {
        tracing::info!("Config changed, full rebuild required");
        return Ok(ChangeSet::full_rebuild());
    }

    let mut changeset = ChangeSet::empty();

    // Track all tags and categories from changed posts
    let mut affected_tags: Vec<String> = Vec::new();
    let mut affected_categories: Vec<String> = Vec::new();

    // Check for changed/new posts
    for (source, hash, tags, categories) in current_posts {
        if let Some(cached) = cache.posts.get(source) {
            if cached.content_hash != *hash {
                tracing::debug!("Post changed: {}", source);
                changeset.changed_posts.push(source.clone());
                // Track affected tags/categories
                affected_tags.extend(tags.clone());
                affected_tags.extend(cached.tags.clone());
                affected_categories.extend(categories.clone());
                affected_categories.extend(cached.categories.clone());
            }
        } else {
            // New post
            tracing::debug!("New post: {}", source);
            changeset.changed_posts.push(source.clone());
            affected_tags.extend(tags.clone());
            affected_categories.extend(categories.clone());
            changeset.rebuild_index = true;
            changeset.rebuild_archives = true;
        }
    }

    // Check for deleted posts
    let current_sources: std::collections::HashSet<_> =
        current_posts.iter().map(|(s, _, _, _)| s.clone()).collect();

    for source in cache.posts.keys() {
        if !current_sources.contains(source) {
            tracing::debug!("Deleted post: {}", source);
            changeset.deleted_posts.push(source.clone());
            if let Some(cached) = cache.posts.get(source) {
                affected_tags.extend(cached.tags.clone());
                affected_categories.extend(cached.categories.clone());
            }
            changeset.rebuild_index = true;
            changeset.rebuild_archives = true;
        }
    }

    // Check for changed/new pages
    for (source, hash) in current_pages {
        if let Some(cached) = cache.pages.get(source) {
            if cached.content_hash != *hash {
                tracing::debug!("Page changed: {}", source);
                changeset.changed_pages.push(source.clone());
            }
        } else {
            tracing::debug!("New page: {}", source);
            changeset.changed_pages.push(source.clone());
        }
    }

    // Check for deleted pages
    let current_page_sources: std::collections::HashSet<_> =
        current_pages.iter().map(|(s, _)| s.clone()).collect();

    for source in cache.pages.keys() {
        if !current_page_sources.contains(source) {
            tracing::debug!("Deleted page: {}", source);
            changeset.deleted_pages.push(source.clone());
        }
    }

    // Determine tag/category rebuild scope
    if !affected_tags.is_empty() {
        affected_tags.sort();
        affected_tags.dedup();
        changeset.rebuild_tags = RebuildScope::Specific(affected_tags);
    }

    if !affected_categories.is_empty() {
        affected_categories.sort();
        affected_categories.dedup();
        changeset.rebuild_categories = RebuildScope::Specific(affected_categories);
    }

    // If post count changed, need to rebuild index
    if current_posts.len() != cache.post_count {
        changeset.rebuild_index = true;
        changeset.rebuild_archives = true;
    }

    Ok(changeset)
}

/// Update cache with current state
pub fn update_cache(
    cache: &mut CacheDb,
    base_dir: &Path,
    theme_dir: &Path,
    posts: &[(String, u64, String, Vec<String>, Vec<String>)], // (source, hash, output_path, tags, categories)
    pages: &[(String, u64, String)],                           // (source, hash, output_path)
) -> Result<()> {
    cache.version = CacheDb::VERSION;

    // Update theme hash
    cache.theme_hash = if theme_dir.exists() {
        hash_directory(theme_dir)?
    } else {
        0
    };

    // Update config hash
    let config_path = base_dir.join("_config.yml");
    cache.config_hash = if config_path.exists() {
        hash_file(&config_path)?
    } else {
        0
    };

    // Update posts
    cache.posts.clear();
    for (source, hash, output_path, tags, categories) in posts {
        cache.posts.insert(
            source.clone(),
            CacheEntry {
                content_hash: *hash,
                mtime: 0, // We use content hash, not mtime
                output_path: output_path.clone(),
                tags: tags.clone(),
                categories: categories.clone(),
            },
        );
    }

    // Update pages
    cache.pages.clear();
    for (source, hash, output_path) in pages {
        cache.pages.insert(
            source.clone(),
            CacheEntry {
                content_hash: *hash,
                mtime: 0,
                output_path: output_path.clone(),
                tags: Vec::new(),
                categories: Vec::new(),
            },
        );
    }

    cache.post_count = posts.len();

    // Calculate overall tags/categories hash
    let all_tags: Vec<_> = posts
        .iter()
        .flat_map(|(_, _, _, tags, _)| tags.clone())
        .collect();
    let all_cats: Vec<_> = posts
        .iter()
        .flat_map(|(_, _, _, _, cats)| cats.clone())
        .collect();
    cache.tags_hash = hash_string_set(&all_tags);
    cache.categories_hash = hash_string_set(&all_cats);

    Ok(())
}
