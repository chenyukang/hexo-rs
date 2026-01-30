//! JavaScript Runtime for EJS Templates
//!
//! This module uses QuickJS to execute JavaScript code in EJS templates.
//! QuickJS is a fast, lightweight JavaScript engine.

use quick_js::{Context, JsValue};

/// JavaScript runtime for executing EJS templates
pub struct JsRuntime {
    context: Context,
}

impl JsRuntime {
    /// Create a new JavaScript runtime
    pub fn new() -> Result<Self, String> {
        let context =
            Context::new().map_err(|e| format!("Failed to create JS context: {:?}", e))?;
        Ok(Self { context })
    }

    /// Initialize the runtime with template context and helpers
    pub fn init_context(&self, context_json: &str) -> Result<(), String> {
        // Parse the context JSON to extract all top-level keys
        let extra_vars = Self::generate_extra_vars(context_json);

        // Parse context JSON and set as global variables
        let init_code = format!(
            r#"
            var __ctx = {};
            var config = __ctx.config || {{}};
            var site = __ctx.site || {{}};
            var page = __ctx.page || {{}};
            var theme = __ctx.theme || {{}};
            var body = __ctx.body || '';
            
            // Set additional variables from context (for partials)
            {}
            
            // Helper function to wrap date string with Hexo-compatible methods
            function __wrapDate(dateVal) {{
                if (!dateVal) return null;
                var dateStr = String(dateVal);
                // Parse date string to get numeric value for comparison
                // Format: YYYY-MM-DD
                var numericValue = 0;
                if (dateStr.length >= 10) {{
                    var year = parseInt(dateStr.substring(0, 4)) || 0;
                    var month = parseInt(dateStr.substring(5, 7)) || 0;
                    var day = parseInt(dateStr.substring(8, 10)) || 0;
                    // Create a comparable numeric value (not actual timestamp, but comparable)
                    numericValue = year * 10000 + month * 100 + day;
                }}
                return {{
                    _date: dateStr,
                    _numericValue: numericValue,
                    toString: function() {{ return this._date; }},
                    valueOf: function() {{ return this._numericValue; }},
                    year: function() {{ return parseInt(this._date.substring(0, 4)) || 0; }},
                    month: function() {{ return parseInt(this._date.substring(5, 7)) || 0; }},
                    date: function() {{ return parseInt(this._date.substring(8, 10)) || 0; }},
                    format: function(fmt) {{
                        fmt = fmt || 'YYYY-MM-DD';
                        return fmt
                            .replace('YYYY', this._date.substring(0, 4))
                            .replace('MM', this._date.substring(5, 7))
                            .replace('DD', this._date.substring(8, 10));
                    }}
                }};
            }}
            
            // Add Hexo-compatible methods to site.posts array
            if (site.posts && Array.isArray(site.posts)) {{
                // Add .each() method (Hexo-style iteration)
                site.posts.each = function(fn) {{
                    for (var i = 0; i < this.length; i++) {{
                        fn(this[i], i);
                    }}
                }};
                // Add .count() method
                site.posts.count = function() {{
                    return this.length;
                }};
                // Wrap each post's date with Hexo-compatible methods
                for (var i = 0; i < site.posts.length; i++) {{
                    var post = site.posts[i];
                    if (post.date && typeof post.date === 'string') {{
                        post.date = __wrapDate(post.date);
                    }}
                }}
            }}
            
            // Add Hexo-compatible methods to site.pages array
            if (site.pages && Array.isArray(site.pages)) {{
                site.pages.each = function(fn) {{
                    for (var i = 0; i < this.length; i++) {{
                        fn(this[i], i);
                    }}
                }};
                site.pages.count = function() {{
                    return this.length;
                }};
            }}
            
            // Also wrap page.date if it exists
            if (page.date && typeof page.date === 'string') {{
                page.date = __wrapDate(page.date);
            }}
            
            // HTML escape helper
            function __escape(s) {{
                if (s === null || s === undefined) return "";
                return String(s)
                    .replace(/&/g, "&amp;")
                    .replace(/</g, "&lt;")
                    .replace(/>/g, "&gt;")
                    .replace(/"/g, "&quot;");
            }}
            
            // url_for helper
            function url_for(path) {{
                if (!path) return '/';
                path = String(path);
                if (path.startsWith('/') || path.startsWith('http')) {{
                    return path;
                }}
                var root = (config && config.root) || '/';
                if (!root.endsWith('/')) root += '/';
                return root + path;
            }}
            
            // date helper - format a date string
            function date(dateVal, format) {{
                if (!dateVal) return '';
                var dateStr = typeof dateVal === 'object' ? (dateVal._date || String(dateVal)) : String(dateVal);
                if (dateStr.length < 10) return dateStr;
                
                format = format || 'YYYY-MM-DD';
                var year = dateStr.substring(0, 4);
                var month = dateStr.substring(5, 7);
                var day = dateStr.substring(8, 10);
                
                return format
                    .replace('YYYY', year)
                    .replace('MM', month)
                    .replace('DD', day);
            }}
            
            // partial helper - generates a placeholder that will be replaced by Rust
            function partial(name, locals) {{
                // Convert locals to JSON, unwrapping date objects
                var localsJson = locals ? JSON.stringify(locals, function(key, value) {{
                    // Unwrap date objects back to strings
                    if (value && typeof value === 'object' && value._date) {{
                        return value._date;
                    }}
                    return value;
                }}) : '{{}}';
                // Use URL encoding which handles Unicode correctly
                return '<!--PARTIAL:' + name + ':' + encodeURIComponent(localsJson) + '-->';
            }}
            
            // __ translation helper
            function __(key) {{
                if (typeof __translations === 'object' && __translations[key]) {{
                    return __translations[key];
                }}
                return key;
            }}
            
            // Page type helpers
            function is_home() {{ return page && page.is_home === true; }}
            function is_post() {{ return page && page.is_post === true; }}
            function is_archive() {{ return page && page.is_archive === true; }}
            function is_category() {{ return page && page.is_category === true; }}
            function is_tag() {{ return page && page.is_tag === true; }}
            
            // Make page.path available as 'path' variable (used by some themes)
            var path = (page && page.path) || '';
            
            // Make page.tags available as 'tags' variable if not already defined
            var tags = (page && page.tags) || [];
            if (tags && !tags.each) {{
                tags.each = function(fn) {{
                    for (var i = 0; i < this.length; i++) {{
                        fn(this[i], i);
                    }}
                }};
            }}
            
            // String helpers
            function trim_words(str, count) {{
                if (!str) return '';
                var words = String(str).split(/\s+/);
                return words.slice(0, count || 100).join(' ');
            }}
            
            function strip_html(str) {{
                if (!str) return '';
                return String(str).replace(/<[^>]*>/g, '');
            }}
            
            // toc helper - generate table of contents from HTML content
            // Returns empty string for now - actual TOC generation is complex
            function toc(content, options) {{
                // Simplified: just return empty string
                // The theme checks if toc() !== "" to decide whether to show catalog
                return '';
            }}
            
            // favicon_tag helper - generate link tag for favicon
            function favicon_tag(path) {{
                if (!path) return '';
                var href = url_for(path);
                return '<link rel="icon" href="' + href + '">';
            }}
            
            // css helper - generate link tag for stylesheet
            function css(path) {{
                if (!path) return '';
                // Handle array of paths
                if (Array.isArray(path)) {{
                    var result = '';
                    for (var i = 0; i < path.length; i++) {{
                        var href = url_for(path[i]);
                        if (!href.endsWith('.css')) href += '.css';
                        result += '<link rel="stylesheet" href="' + href + '">\n';
                    }}
                    return result;
                }}
                var href = url_for(path);
                if (!href.endsWith('.css')) href += '.css';
                return '<link rel="stylesheet" href="' + href + '">';
            }}
            
            // js helper - generate script tag
            function js(path) {{
                if (!path) return '';
                // Handle array of paths
                if (Array.isArray(path)) {{
                    var result = '';
                    for (var i = 0; i < path.length; i++) {{
                        var src = url_for(path[i]);
                        if (!src.endsWith('.js')) src += '.js';
                        result += '<script src="' + src + '"><\/script>\n';
                    }}
                    return result;
                }}
                var src = url_for(path);
                if (!src.endsWith('.js')) src += '.js';
                return '<script src="' + src + '"><\/script>';
            }}
            
            // gravatar helper - generate gravatar URL
            function gravatar(email, size) {{
                size = size || 80;
                return 'https://www.gravatar.com/avatar/?s=' + size + '&d=mm';
            }}
            
            // wordcount / min2read helpers
            function wordcount(content) {{
                if (!content) return 0;
                var text = strip_html(String(content));
                var words = text.split(/[ \t\n\r]+/);
                var count = 0;
                for (var i = 0; i < words.length; i++) {{
                    if (words[i].length > 0) count++;
                }}
                return count;
            }}
            
            function min2read(content, wordsPerMinute) {{
                wordsPerMinute = wordsPerMinute || 200;
                var words = wordcount(content);
                return Math.ceil(words / wordsPerMinute);
            }}
            "#,
            context_json, extra_vars
        );

        self.context
            .eval(&init_code)
            .map_err(|e| format!("Failed to init context: {:?}", e))?;

        Ok(())
    }

    /// Generate JavaScript variable declarations for extra context keys
    /// This allows partials to receive custom variables like `year` and `posts`
    fn generate_extra_vars(context_json: &str) -> String {
        // List of keys that are already handled as standard variables
        let standard_keys = ["config", "site", "page", "theme", "body"];

        // List of JavaScript reserved words that cannot be used as variable names
        let reserved_words = [
            "break",
            "case",
            "catch",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "finally",
            "for",
            "function",
            "if",
            "in",
            "instanceof",
            "new",
            "return",
            "switch",
            "this",
            "throw",
            "try",
            "typeof",
            "var",
            "void",
            "while",
            "with",
            "class",
            "const",
            "enum",
            "export",
            "extends",
            "import",
            "super",
            "implements",
            "interface",
            "let",
            "package",
            "private",
            "protected",
            "public",
            "static",
            "yield",
            "null",
            "true",
            "false",
        ];

        // Parse the JSON to extract top-level keys
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(context_json) {
            if let Some(obj) = value.as_object() {
                let mut vars = Vec::new();
                for key in obj.keys() {
                    // Skip standard keys, private keys (starting with _), and reserved words
                    if standard_keys.contains(&key.as_str())
                        || key.starts_with('_')
                        || reserved_words.contains(&key.as_str())
                    {
                        continue;
                    }

                    // Validate that key is a valid JavaScript identifier
                    // Must start with letter, underscore, or dollar sign
                    // And contain only alphanumeric, underscore, or dollar sign
                    let is_valid_identifier = key
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
                        .unwrap_or(false)
                        && key
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');

                    if !is_valid_identifier {
                        continue;
                    }

                    // Generate: var {key} = __ctx.{key};
                    // Also wrap posts array items with date methods if it's "posts"
                    if key == "posts" {
                        vars.push(format!(
                            r#"var {key} = __ctx.{key} || [];
            if (Array.isArray({key})) {{
                for (var __i = 0; __i < {key}.length; __i++) {{
                    if ({key}[__i].date && typeof {key}[__i].date === 'string') {{
                        {key}[__i].date = __wrapDate({key}[__i].date);
                    }}
                }}
            }}"#,
                            key = key
                        ));
                    } else {
                        vars.push(format!("var {} = __ctx.{};", key, key));
                    }
                }
                return vars.join("\n            ");
            }
        }
        String::new()
    }

    /// Compile EJS template to JavaScript code
    pub fn compile_ejs(template: &str) -> String {
        let mut js_code = String::new();
        js_code.push_str("(function() {\n");
        js_code.push_str("var __output = '';\n");

        let mut pos = 0;
        let chars: Vec<char> = template.chars().collect();
        let len = chars.len();

        while pos < len {
            // Look for <%
            if pos + 1 < len && chars[pos] == '<' && chars[pos + 1] == '%' {
                // Find the closing %>
                let start = pos + 2;
                let mut end = start;
                while end + 1 < len && !(chars[end] == '%' && chars[end + 1] == '>') {
                    end += 1;
                }

                if end + 1 < len {
                    let tag_content: String = chars[start..end].iter().collect();
                    let tag_content = tag_content.trim();

                    if tag_content.starts_with('-') {
                        // Raw output: <%- expr %>
                        let expr = tag_content[1..].trim();
                        if !expr.is_empty() {
                            js_code.push_str(&format!("__output += ({});\n", expr));
                        }
                    } else if tag_content.starts_with('=') {
                        // Escaped output: <%= expr %>
                        let expr = tag_content[1..].trim();
                        if !expr.is_empty() {
                            js_code.push_str(&format!("__output += __escape({});\n", expr));
                        }
                    } else if tag_content.starts_with('#') {
                        // Comment: <%# ... %> - ignore
                    } else {
                        // Code block: <% code %>
                        if !tag_content.is_empty() {
                            js_code.push_str(tag_content);
                            js_code.push('\n');
                        }
                    }

                    pos = end + 2;
                } else {
                    // No closing tag found, treat as text
                    js_code.push_str(&format!(
                        "__output += {};\n",
                        Self::escape_js_string(&chars[pos].to_string())
                    ));
                    pos += 1;
                }
            } else {
                // Regular text - collect until next <%
                let text_start = pos;
                while pos < len && !(pos + 1 < len && chars[pos] == '<' && chars[pos + 1] == '%') {
                    pos += 1;
                }
                let text: String = chars[text_start..pos].iter().collect();
                if !text.is_empty() {
                    js_code.push_str(&format!("__output += {};\n", Self::escape_js_string(&text)));
                }
            }
        }

        js_code.push_str("return __output;\n");
        js_code.push_str("})()");
        js_code
    }

    /// Escape a string for use in JavaScript
    fn escape_js_string(s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 2);
        result.push('"');
        for c in s.chars() {
            match c {
                '"' => result.push_str("\\\""),
                '\\' => result.push_str("\\\\"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                _ => result.push(c),
            }
        }
        result.push('"');
        result
    }

    /// Execute JavaScript code and return the result as string
    pub fn execute(&self, code: &str) -> Result<String, String> {
        let result = self
            .context
            .eval(code)
            .map_err(|e| format!("JS execution error: {:?}", e))?;

        match result {
            JsValue::String(s) => Ok(s),
            JsValue::Int(n) => Ok(n.to_string()),
            JsValue::Float(n) => Ok(n.to_string()),
            JsValue::Bool(b) => Ok(b.to_string()),
            JsValue::Null | JsValue::Undefined => Ok(String::new()),
            _ => Ok(String::new()),
        }
    }

    /// Render an EJS template with the given context
    pub fn render_template(&self, template: &str, context_json: &str) -> Result<String, String> {
        // Initialize context
        self.init_context(context_json)?;

        // Compile template to JS
        let js_code = Self::compile_ejs(template);

        // Execute and return result
        self.execute(&js_code)
    }

    /// Render an EJS template with partials support
    pub fn render_with_partials<F>(
        &self,
        template: &str,
        context_json: &str,
        partial_renderer: F,
    ) -> Result<String, String>
    where
        F: Fn(&str, &str) -> Result<String, String>,
    {
        // First pass: render the template (partials become placeholders)
        let output = self.render_template(template, context_json)?;

        // Second pass: replace partial placeholders with rendered content
        Self::process_partial_placeholders(&output, &partial_renderer)
    }

    /// Process partial placeholders in the rendered output
    fn process_partial_placeholders<F>(output: &str, partial_renderer: &F) -> Result<String, String>
    where
        F: Fn(&str, &str) -> Result<String, String>,
    {
        use regex::Regex;

        // Match partial placeholders - the encoded content can contain any characters except the closing -->
        let re = Regex::new(r"<!--PARTIAL:([^:]+):(.*?)-->").map_err(|e| e.to_string())?;

        let mut result = output.to_string();

        // Keep processing until no more partials (handles nested partials)
        let mut max_iterations = 10; // Prevent infinite loops
        while result.contains("<!--PARTIAL:") && max_iterations > 0 {
            max_iterations -= 1;

            let mut new_result = result.clone();
            let mut offset = 0i64;

            for cap in re.captures_iter(&result) {
                let full_match = cap.get(0).unwrap();
                let name = &cap[1];
                let locals_encoded = &cap[2];

                // Decode URL-encoded locals
                let locals_json =
                    Self::decode_url_encoded(locals_encoded).unwrap_or_else(|_| "{}".to_string());

                // Render the partial
                match partial_renderer(name, &locals_json) {
                    Ok(rendered) => {
                        let start = (full_match.start() as i64 + offset) as usize;
                        let end = (full_match.end() as i64 + offset) as usize;
                        let old_len = end - start;
                        let new_len = rendered.len();

                        new_result.replace_range(start..end, &rendered);
                        offset += new_len as i64 - old_len as i64;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to render partial {}: {}", name, e);
                    }
                }
            }

            result = new_result;
        }

        Ok(result)
    }

    /// Decode URL-encoded string (handles Unicode correctly)
    fn decode_url_encoded(input: &str) -> Result<String, String> {
        use percent_encoding::percent_decode_str;
        percent_decode_str(input)
            .decode_utf8()
            .map(|s| s.into_owned())
            .map_err(|e| e.to_string())
    }
}

impl Default for JsRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create JsRuntime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple_template() {
        let template = "Hello <%= name %>!";
        let js_code = JsRuntime::compile_ejs(template);
        assert!(js_code.contains("__output += __escape(name)"));
        assert!(js_code.contains("\"Hello \""));
    }

    #[test]
    fn test_compile_raw_output() {
        let template = "<%- content %>";
        let js_code = JsRuntime::compile_ejs(template);
        assert!(js_code.contains("__output += (content)"));
    }

    #[test]
    fn test_compile_code_block() {
        let template = "<% if (true) { %>yes<% } %>";
        let js_code = JsRuntime::compile_ejs(template);
        assert!(js_code.contains("if (true) {"));
        assert!(js_code.contains("\"yes\""));
    }

    #[test]
    fn test_execute_simple() {
        let runtime = JsRuntime::new().unwrap();
        let result = runtime.execute("1 + 2").unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn test_render_template_with_context() {
        let runtime = JsRuntime::new().unwrap();
        let template = "Hello <%= config.title %>!";
        let context = r#"{"config": {"title": "World"}}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_render_template_with_loop() {
        let runtime = JsRuntime::new().unwrap();
        let template = r#"<% for (var i = 0; i < 3; i++) { %><%= i %><% } %>"#;
        let context = r#"{}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert_eq!(result, "012");
    }

    #[test]
    fn test_render_template_with_array_foreach() {
        let runtime = JsRuntime::new().unwrap();
        let template =
            r#"<% var items = [1, 2, 3]; items.forEach(function(item) { %><%= item %><% }); %>"#;
        let context = r#"{}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert_eq!(result, "123");
    }

    #[test]
    fn test_archive_pattern() {
        let runtime = JsRuntime::new().unwrap();
        let template = r#"<% var years = {}; %>
<% var posts = [{date: "2024-01-01", title: "A"}, {date: "2023-05-01", title: "B"}]; %>
<% posts.forEach(function(post) {
    var y = post.date.substring(0, 4);
    if (!years[y]) { years[y] = []; }
    years[y].push(post);
}); %>
<% for (var year in years) { %>
Year: <%= year %> (<%= years[year].length %>)
<% } %>"#;
        let context = r#"{}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert!(result.contains("Year: 2024"));
        assert!(result.contains("Year: 2023"));
    }

    #[test]
    fn test_hexo_posts_each_method() {
        let runtime = JsRuntime::new().unwrap();
        // Test that site.posts.each() works like Hexo
        let template = r#"<% var result = []; site.posts.each(function(post) { result.push(post.title); }); %><%= result.join(",") %>"#;
        let context = r#"{"site": {"posts": [{"title": "A"}, {"title": "B"}, {"title": "C"}]}}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert_eq!(result, "A,B,C");
    }

    #[test]
    fn test_hexo_posts_count_method() {
        let runtime = JsRuntime::new().unwrap();
        let template = r#"Total: <%= site.posts.count() %>"#;
        let context = r#"{"site": {"posts": [{"title": "A"}, {"title": "B"}]}}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert_eq!(result, "Total: 2");
    }

    #[test]
    fn test_hexo_date_year_method() {
        let runtime = JsRuntime::new().unwrap();
        // Test that post.date.year() works like Hexo
        let template = r#"<% var years = []; site.posts.each(function(post) { years.push(post.date.year()); }); %><%= years.join(",") %>"#;
        let context = r#"{"site": {"posts": [{"date": "2024-03-15", "title": "A"}, {"date": "2023-01-01", "title": "B"}]}}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert_eq!(result, "2024,2023");
    }

    #[test]
    fn test_hexo_archive_style() {
        // This test mimics the actual vexo theme archive.ejs pattern
        let runtime = JsRuntime::new().unwrap();
        let template = r#"<% var years = {}; %>
<% site.posts.each(function(post) {
  var y = post.date.year();
  if (!years[y]) { years[y] = []; }
  years[y].push(post);
}); %>
<% for (var year in years) { %>
Year: <%= year %> Count: <%= years[year].length %>
<% } %>"#;
        let context = r#"{"site": {"posts": [
            {"date": "2024-03-15", "title": "Post 1"},
            {"date": "2024-01-10", "title": "Post 2"},
            {"date": "2023-06-20", "title": "Post 3"}
        ]}}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert!(result.contains("Year: 2024"));
        assert!(result.contains("Year: 2023"));
        assert!(result.contains("Count: 2")); // 2024 has 2 posts
        assert!(result.contains("Count: 1")); // 2023 has 1 post
    }

    #[test]
    fn test_partial_variables() {
        // Test that extra context variables (like year, posts) are available as globals
        let runtime = JsRuntime::new().unwrap();
        let template = r#"Year: <%= year %>, Posts: <%= posts.length %>"#;
        let context = r#"{"year": "2024", "posts": [{"title": "A"}, {"title": "B"}], "config": {}, "site": {}, "page": {}, "theme": {}}"#;

        let result = runtime.render_template(template, context).unwrap();
        assert_eq!(result, "Year: 2024, Posts: 2");
    }

    #[test]
    fn test_generate_extra_vars() {
        let context = r#"{"year": "2024", "posts": [], "config": {}, "site": {}}"#;
        let extra_vars = JsRuntime::generate_extra_vars(context);

        // Should contain year but not config or site
        assert!(extra_vars.contains("var year = __ctx.year;"));
        assert!(extra_vars.contains("var posts = __ctx.posts"));
        assert!(!extra_vars.contains("var config"));
        assert!(!extra_vars.contains("var site"));
    }

    #[test]
    fn test_partial_placeholder_generation() {
        // Test that partial() generates correct placeholder with encoded locals
        let runtime = JsRuntime::new().unwrap();
        let template = r#"<%- partial('_partial/archive', { year: "2024", posts: [1,2,3] }) %>"#;
        let context = r#"{"config": {}, "site": {}, "page": {}, "theme": {}}"#;

        let result = runtime.render_template(template, context).unwrap();
        // Should contain a PARTIAL placeholder
        assert!(result.contains("<!--PARTIAL:_partial/archive:"));
        assert!(result.contains("-->"));

        // Extract and decode the URL-encoded part
        if let Some(start) = result.find("<!--PARTIAL:_partial/archive:") {
            let after_prefix = &result[start + "<!--PARTIAL:_partial/archive:".len()..];
            if let Some(end) = after_prefix.find("-->") {
                let encoded = &after_prefix[..end];
                let decoded = JsRuntime::decode_url_encoded(encoded).unwrap();
                println!("Decoded locals: {}", decoded);
                assert!(decoded.contains("year"));
                assert!(decoded.contains("2024"));
                assert!(decoded.contains("posts"));
            }
        }
    }
}
