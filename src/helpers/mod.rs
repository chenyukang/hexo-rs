//! Helper functions for templates
//!
//! These functions are available in EJS templates and provide
//! common functionality like URL generation, HTML helpers, etc.

mod date;
mod html;
mod list;
mod url;

use std::collections::HashMap;

pub use date::*;
pub use html::*;
pub use list::*;
pub use url::*;

use crate::config::SiteConfig;

/// Collection of all helper functions
pub struct Helpers {
    config: SiteConfig,
}

impl Helpers {
    /// Create a new helpers instance
    pub fn new(config: SiteConfig) -> Self {
        Self { config }
    }

    /// Get url_for helper
    pub fn url_for(&self, path: &str) -> String {
        url_for(&self.config, path)
    }

    /// Get full_url_for helper
    pub fn full_url_for(&self, path: &str) -> String {
        full_url_for(&self.config, path)
    }

    /// Get relative_url helper
    pub fn relative_url(&self, from: &str, to: &str) -> String {
        relative_url(from, to)
    }

    /// Get css helper
    pub fn css(&self, path: &str) -> String {
        css(&self.config, path)
    }

    /// Get js helper
    pub fn js(&self, path: &str) -> String {
        js(&self.config, path)
    }

    /// Get link_to helper
    pub fn link_to(&self, path: &str, text: &str) -> String {
        link_to(&self.config, path, text, false)
    }

    /// Get image_tag helper
    pub fn image_tag(&self, path: &str, alt: Option<&str>) -> String {
        image_tag(&self.config, path, alt, None)
    }

    /// Format a date
    pub fn date(&self, date: &chrono::DateTime<chrono::Local>, format: Option<&str>) -> String {
        format_date(date, format.unwrap_or(&self.config.date_format))
    }

    /// Get all helpers as a map for template context
    pub fn as_context_map(&self) -> HashMap<String, String> {
        // In a full implementation, this would register callable functions
        HashMap::new()
    }
}
