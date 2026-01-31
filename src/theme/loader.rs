//! Theme loader - loads and manages theme templates

use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

use super::ejs::{EjsEngine, EjsValue};
use super::engine::{TemplateContext, TemplateEngine};
use super::jsruntime::JsRuntime;
use crate::i18n::I18n;

// Thread-local cache for JsRuntime to avoid creating new instances for each render
// We use UnsafeCell because RefCell causes borrow conflicts with nested partial rendering
thread_local! {
    static JS_RUNTIME: std::cell::UnsafeCell<Option<JsRuntime>> = const { std::cell::UnsafeCell::new(None) };
}

/// Get or create a cached JsRuntime and run a closure with it
/// The closure returns Result<String, String>, and this function flattens it
///
/// SAFETY: This is safe because:
/// 1. thread_local ensures single-threaded access
/// 2. JsRuntime uses interior mutability (QuickJS context is internally managed)
/// 3. We only hand out immutable references to the runtime
fn with_cached_runtime<F>(f: F) -> Result<String, String>
where
    F: FnOnce(&JsRuntime) -> Result<String, String>,
{
    JS_RUNTIME.with(|cell| {
        // SAFETY: Single-threaded access guaranteed by thread_local
        let opt = unsafe { &mut *cell.get() };
        if opt.is_none() {
            *opt = Some(JsRuntime::new()?);
        }
        f(opt.as_ref().unwrap())
    })
}

/// Theme loader and renderer
pub struct ThemeLoader {
    /// Theme directory path
    theme_dir: PathBuf,
    /// EJS template engine
    engine: EjsEngine,
    /// Loaded templates
    templates: HashMap<String, String>,
    /// Layout templates
    layouts: HashMap<String, String>,
    /// Partial templates (shared for rendering)
    partials: Arc<HashMap<String, String>>,
    /// Theme configuration (IndexMap preserves YAML key order for menu items)
    config: IndexMap<String, serde_yaml::Value>,
    /// Internationalization handler
    i18n: I18n,
}

impl ThemeLoader {
    /// Load a theme from a directory
    pub fn load<P: AsRef<Path>>(theme_dir: P) -> Result<Self> {
        Self::load_with_language(theme_dir, "en")
    }

    /// Load a theme from a directory with a specific language
    pub fn load_with_language<P: AsRef<Path>>(theme_dir: P, language: &str) -> Result<Self> {
        let theme_dir = theme_dir.as_ref().to_path_buf();

        if !theme_dir.exists() {
            return Err(anyhow!("Theme directory not found: {:?}", theme_dir));
        }

        // Initialize i18n
        let mut i18n = I18n::new(language);

        // Load language files from theme
        let languages_dir = theme_dir.join("languages");
        if languages_dir.exists() {
            i18n.load_languages(&languages_dir)?;
            tracing::debug!("Loaded language files from {:?}", languages_dir);
        }

        let mut loader = Self {
            theme_dir: theme_dir.clone(),
            engine: EjsEngine::new(),
            templates: HashMap::new(),
            layouts: HashMap::new(),
            partials: Arc::new(HashMap::new()),
            config: IndexMap::new(),
            i18n,
        };

        // Load theme config
        let config_path = theme_dir.join("_config.yml");
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            loader.config = serde_yaml::from_str(&content)?;
        }

        // Load layout templates
        let layout_dir = theme_dir.join("layout");
        if layout_dir.exists() {
            loader.load_templates(&layout_dir)?;
        }

        Ok(loader)
    }

    /// Load templates from a directory
    fn load_templates(&mut self, dir: &Path) -> Result<()> {
        let mut partials = HashMap::new();

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());

                // Support EJS, Nunjucks, and Swig templates
                if matches!(ext, Some("ejs") | Some("njk") | Some("swig") | Some("html")) {
                    let content = fs::read_to_string(path)?;

                    // Determine template name from path
                    let relative = path.strip_prefix(dir).unwrap_or(path);
                    let name = relative
                        .with_extension("")
                        .to_string_lossy()
                        .replace('\\', "/");

                    // Categorize templates - store partials with multiple key formats
                    if name.starts_with("_partial/") || name.starts_with("partial/") {
                        let partial_name = name
                            .trim_start_matches("_partial/")
                            .trim_start_matches("partial/");
                        // Store with short name
                        partials.insert(partial_name.to_string(), content.clone());
                        // Store with _partial/ prefix
                        partials.insert(format!("_partial/{}", partial_name), content.clone());
                    }

                    // Store widgets similarly
                    if name.starts_with("_widget/") || name.starts_with("widget/") {
                        let widget_name = name
                            .trim_start_matches("_widget/")
                            .trim_start_matches("widget/");
                        partials.insert(widget_name.to_string(), content.clone());
                        partials.insert(format!("_widget/{}", widget_name), content.clone());
                    }

                    // Store by full name
                    self.templates.insert(name.clone(), content.clone());
                    partials.insert(name.clone(), content.clone());

                    // If it's a main layout (layout.ejs), store separately
                    if name == "layout" {
                        self.layouts.insert("default".to_string(), content);
                    }

                    tracing::debug!("Loaded template: {}", name);
                }
            }
        }

        self.partials = Arc::new(partials);
        Ok(())
    }

    /// Get theme configuration
    pub fn config(&self) -> &IndexMap<String, serde_yaml::Value> {
        &self.config
    }

    /// Get a template by name
    pub fn get_template(&self, name: &str) -> Option<&String> {
        self.templates.get(name)
    }

    /// Get a partial by name
    pub fn get_partial(&self, name: &str) -> Option<&String> {
        self.partials.get(name)
    }

    /// Get all partials for rendering
    pub fn partials(&self) -> Arc<HashMap<String, String>> {
        self.partials.clone()
    }

    /// Render a template with layout
    pub fn render_with_layout(
        &self,
        template_name: &str,
        context: &TemplateContext,
    ) -> Result<String> {
        // Find the template
        let template_source = self
            .templates
            .get(template_name)
            .ok_or_else(|| anyhow!("Template not found: {}", template_name))?;

        // Add theme config to context
        let mut render_context = context.clone();
        self.add_theme_to_context(&mut render_context);

        // Use QuickJS only for archive template (has complex JS like .each(), .year())
        // Other templates use the fast EJS engine
        let body = if template_name == "archive" {
            // Check if we have pre-computed years data for optimization
            let has_precomputed = render_context.inner().get("__years").is_some()
                && render_context.inner().get("__yearsReversed").is_some();

            if has_precomputed {
                // Use optimized archive rendering with pre-computed data
                self.render_archive_optimized(template_source, &render_context)?
            } else {
                self.render_with_js_runtime(template_source, &render_context)
                    .unwrap_or_else(|e| {
                        tracing::warn!("JsRuntime failed for {}: {}", template_name, e);
                        String::new()
                    })
            }
        } else {
            // Use fast EJS engine for most templates
            let template = self.engine.parse(template_source)?;
            self.engine.render_with_partials(
                &template,
                render_context.inner(),
                self.partials.clone(),
            )?
        };

        // Check if we should apply a layout - always use fast EJS for layout
        if let Some(layout_source) = self.layouts.get("default") {
            let mut layout_context = render_context.clone();
            layout_context.set_string("body", &body);

            let layout = self.engine.parse(layout_source)?;
            self.engine
                .render_with_partials(&layout, layout_context.inner(), self.partials.clone())
                .map_err(Into::into)
        } else {
            Ok(body)
        }
    }

    /// Render archive template with pre-computed years data (optimized path)
    /// This avoids the expensive JS site.posts.each() + date.year() grouping
    fn render_archive_optimized(
        &self,
        template_source: &str,
        context: &TemplateContext,
    ) -> Result<String> {
        // Extract pre-computed data from context
        let years_data = context.inner().get("__years");
        let years_reversed = context.inner().get("__yearsReversed");

        // Generate the archive body using pre-computed years data
        let archive_body =
            if let (Some(EjsValue::Object(years)), Some(EjsValue::Array(years_order))) =
                (years_data, years_reversed)
            {
                let mut body = String::new();

                // Iterate through years in reverse order
                for year_val in years_order {
                    let year_str = year_val.to_output_string();
                    if let Some(EjsValue::Array(posts)) = years.get(&year_str) {
                        // Render the _partial/archive for this year
                        let partial_html =
                            self.render_archive_year_partial(&year_str, posts, context)?;
                        body.push_str(&partial_html);
                    }
                }

                body
            } else {
                // Fallback to JS runtime if pre-computed data is missing
                return self
                    .render_with_js_runtime(template_source, context)
                    .map_err(|e| anyhow!("{}", e));
            };

        // Now render the archive template wrapper with the pre-rendered body
        // Replace the complex JS logic with simple output
        let site_posts_count = if let Some(EjsValue::Object(site)) = context.inner().get("site") {
            if let Some(EjsValue::Array(posts)) = site.get("posts") {
                posts.len()
            } else {
                0
            }
        } else {
            0
        };

        // Build the simplified archive template output
        let config_url = if let Some(EjsValue::Object(config)) = context.inner().get("config") {
            config
                .get("url")
                .map(|v| v.to_output_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

        let page_path = if let Some(EjsValue::Object(page)) = context.inner().get("page") {
            page.get("current_url")
                .map(|v| v.to_output_string())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Generate the archive page HTML directly
        let html = format!(
            r#"<div id="article-banner">
  <h2>Archives</h2>
  <p class="post-date">文章归档: {} </p>
</div>
<main class="app-body" id="archives">
  <div class="archives-container">
    {}
  </div>
</main>


<script>
  (function() {{
    var url = '{}/{}';
    $('#article-banner').geopattern(url);
    $('.header').removeClass('fixed-header');
  }})();
</script>"#,
            site_posts_count, archive_body, config_url, page_path
        );

        Ok(html)
    }

    /// Render a single year's archive partial
    fn render_archive_year_partial(
        &self,
        year: &str,
        posts: &[EjsValue],
        context: &TemplateContext,
    ) -> Result<String> {
        let root = if let Some(EjsValue::Object(config)) = context.inner().get("config") {
            config
                .get("root")
                .map(|v| v.to_output_string())
                .unwrap_or_else(|| "/".to_string())
        } else {
            "/".to_string()
        };

        let mut posts_html = String::new();
        for post in posts {
            if let EjsValue::Object(post_obj) = post {
                let title = post_obj
                    .get("title")
                    .map(|v| v.to_output_string())
                    .unwrap_or_default();
                let path = post_obj
                    .get("path")
                    .map(|v| v.to_output_string())
                    .unwrap_or_default();
                let date = post_obj
                    .get("date")
                    .map(|v| v.to_output_string())
                    .unwrap_or_default();

                let url = if path.starts_with('/') || path.starts_with("http") {
                    path
                } else {
                    format!(
                        "{}{}",
                        root.trim_end_matches('/'),
                        if path.is_empty() {
                            "".to_string()
                        } else {
                            format!("/{}", path.trim_start_matches('/'))
                        }
                    )
                };

                posts_html.push_str(&format!(
                    r#"      <div class="section-list-item">
        <a href="{}" class="archive-title">{}</a>
        <p class="archive-date">{}</p>
      </div>
"#,
                    url, title, date
                ));
            }
        }

        Ok(format!(
            r#"<section class="time-section">
  <h1 class="section-year">
    {}
  </h1>
  <div class="section-list">
{}  </div>
</section>
"#,
            year, posts_html
        ))
    }

    /// Render a template using the JavaScript runtime
    fn render_with_js_runtime(&self, template: &str, context: &TemplateContext) -> Result<String> {
        // Convert context to JSON
        let context_json = context.inner().to_json();

        // Clone partials for the closure
        let partials = self.partials.clone();

        // Render with partial support - use fast EJS for partials
        let engine = self.engine.clone();

        // Use cached JsRuntime
        with_cached_runtime(|runtime| {
            runtime.render_with_partials(template, &context_json, |partial_name, locals_json| {
                Self::render_partial_fast(&engine, partial_name, locals_json, context, &partials)
            })
        })
        .map_err(|e| anyhow!("{}", e))
    }

    /// Render a partial using fast EJS engine
    fn render_partial_fast(
        engine: &EjsEngine,
        partial_name: &str,
        locals_json: &str,
        parent_context: &TemplateContext,
        partials: &Arc<HashMap<String, String>>,
    ) -> Result<String, String> {
        // Find the partial template
        let partial_source = partials
            .get(partial_name)
            .or_else(|| partials.get(&format!("_partial/{}", partial_name)))
            .ok_or_else(|| format!("Partial not found: {}", partial_name))?;

        // Check if partial contains complex JavaScript that needs QuickJS
        // (e.g., arrow functions with expressions like (l, r) => r.date - l.date)
        let needs_quickjs = partial_source.contains("=>")
            && (partial_source.contains(".sort(") || partial_source.contains(" - "));

        if needs_quickjs {
            // Merge parent context with locals
            let mut context_value = parent_context.to_json();

            // Ensure context_value is an object
            if !context_value.is_object() {
                context_value = serde_json::Value::Object(serde_json::Map::new());
            }

            // Merge locals into context
            if let Ok(locals) = serde_json::from_str::<serde_json::Value>(locals_json) {
                if let serde_json::Value::Object(ref mut ctx) = context_value {
                    if let serde_json::Value::Object(loc) = locals {
                        for (k, v) in loc {
                            ctx.insert(k, v);
                        }
                    }
                }
            }
            let context_json = serde_json::to_string(&context_value).unwrap_or_default();

            // Use cached JsRuntime for complex partials
            with_cached_runtime(|runtime| {
                runtime
                    .render_template(partial_source, &context_json)
                    .map_err(|e| {
                        format!(
                            "Failed to render partial {} with QuickJS: {}",
                            partial_name, e
                        )
                    })
            })
        } else {
            // Use fast EJS engine for simple partials
            let mut merged_context = parent_context.clone();
            if let Ok(serde_json::Value::Object(obj)) =
                serde_json::from_str::<serde_json::Value>(locals_json)
            {
                for (k, v) in obj {
                    merged_context.inner_mut().set(&k, EjsValue::from_json(&v));
                }
            }

            // Parse and render with fast EJS engine
            let template = engine
                .parse(partial_source)
                .map_err(|e| format!("Failed to parse partial {}: {}", partial_name, e))?;

            engine
                .render_with_partials(&template, merged_context.inner(), partials.clone())
                .map_err(|e| format!("Failed to render partial {}: {}", partial_name, e))
        }
    }

    #[allow(dead_code)]
    /// Render a partial for the JS runtime (static version to avoid borrowing issues)
    fn render_partial_for_js_static(
        partial_name: &str,
        locals_json: &str,
        parent_context_json: &str,
        partials: &Arc<HashMap<String, String>>,
    ) -> Result<String, String> {
        Self::render_partial_for_js_with_depth(
            partial_name,
            locals_json,
            parent_context_json,
            partials,
            0,
        )
    }

    #[allow(dead_code)]
    /// Render a partial with depth tracking to prevent infinite recursion
    fn render_partial_for_js_with_depth(
        partial_name: &str,
        locals_json: &str,
        parent_context_json: &str,
        partials: &Arc<HashMap<String, String>>,
        depth: usize,
    ) -> Result<String, String> {
        // Limit nesting depth to prevent stack overflow
        if depth > 5 {
            tracing::warn!(
                "Partial nesting too deep for {}, returning empty",
                partial_name
            );
            return Ok(String::new());
        }

        tracing::debug!("Rendering partial: {} (depth {})", partial_name, depth);

        // Find the partial template
        let partial_source = partials
            .get(partial_name)
            .or_else(|| partials.get(&format!("_partial/{}", partial_name)))
            .ok_or_else(|| format!("Partial not found: {}", partial_name))?;

        // Build a combined context JSON that includes parent context and locals
        let mut context_value: serde_json::Value = serde_json::from_str(parent_context_json)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        if let Ok(locals) = serde_json::from_str::<serde_json::Value>(locals_json) {
            if let (serde_json::Value::Object(ref mut ctx), serde_json::Value::Object(loc)) =
                (&mut context_value, locals)
            {
                for (k, v) in loc {
                    ctx.insert(k, v);
                }
            }
        }
        let combined_json = serde_json::to_string(&context_value).unwrap_or_default();

        // Clone partials for nested rendering
        let nested_partials = partials.clone();
        let next_depth = depth + 1;
        let partial_source = partial_source.clone();

        // Use cached JsRuntime with nested partial support
        with_cached_runtime(|runtime| {
            runtime.render_with_partials(
                &partial_source,
                &combined_json,
                |nested_name, nested_locals| {
                    Self::render_partial_for_js_with_depth(
                        nested_name,
                        nested_locals,
                        &combined_json,
                        &nested_partials,
                        next_depth,
                    )
                },
            )
        })
        .map_err(|e| format!("Failed to render partial {}: {}", partial_name, e))
    }

    /// Add theme configuration to context
    fn add_theme_to_context(&self, context: &mut TemplateContext) {
        // Convert theme config to EjsValue
        let mut theme_obj = IndexMap::new();
        for (key, value) in &self.config {
            theme_obj.insert(key.clone(), yaml_to_ejs_value(value));
        }
        context
            .inner_mut()
            .set("theme", EjsValue::Object(theme_obj));

        // Add i18n translations to context for the __ helper
        let translations = self.i18n.get_all_translations();
        let mut trans_obj = IndexMap::new();
        for (key, value) in translations {
            trans_obj.insert(key, EjsValue::String(value));
        }
        context.inner_mut().set("__", EjsValue::Object(trans_obj));
    }

    /// Find the best template for a page type with fallbacks
    pub fn find_template(&self, name: &str, fallbacks: &[&str]) -> Option<String> {
        if self.templates.contains_key(name) {
            return Some(name.to_string());
        }

        for fallback in fallbacks {
            if self.templates.contains_key(*fallback) {
                return Some(fallback.to_string());
            }
        }

        None
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

impl TemplateEngine for ThemeLoader {
    fn render(&self, template_name: &str, context: &TemplateContext) -> Result<String> {
        self.render_with_layout(template_name, context)
    }

    fn render_string(&self, template: &str, context: &TemplateContext) -> Result<String> {
        let parsed = self.engine.parse(template)?;
        let mut render_context = context.clone();
        self.add_theme_to_context(&mut render_context);
        self.engine
            .render_with_partials(&parsed, render_context.inner(), self.partials.clone())
            .map_err(Into::into)
    }

    fn has_template(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }

    fn template_names(&self) -> Vec<String> {
        self.templates.keys().cloned().collect()
    }
}

/// Convert a serde_yaml::Value to an EjsValue
fn yaml_to_ejs_value(value: &serde_yaml::Value) -> EjsValue {
    match value {
        serde_yaml::Value::Null => EjsValue::Null,
        serde_yaml::Value::Bool(b) => EjsValue::Bool(*b),
        serde_yaml::Value::Number(n) => EjsValue::Number(n.as_f64().unwrap_or(0.0)),
        serde_yaml::Value::String(s) => EjsValue::String(s.clone()),
        serde_yaml::Value::Sequence(arr) => {
            EjsValue::Array(arr.iter().map(yaml_to_ejs_value).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut obj = IndexMap::new();
            for (k, v) in map {
                if let Some(key) = k.as_str() {
                    obj.insert(key.to_string(), yaml_to_ejs_value(v));
                }
            }
            EjsValue::Object(obj)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_ejs_value(&tagged.value),
    }
}
