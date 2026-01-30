//! EJS Template Engine Implementation
//!
//! This module implements an EJS (Embedded JavaScript) compatible template engine
//! for Rust, using a proper Lexer + Parser + AST architecture.

use chrono::{Datelike, TimeZone, Timelike};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// EJS template parsing and rendering errors
#[derive(Error, Debug)]
pub enum EjsError {
    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("Render error: {0}")]
    RenderError(String),

    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// ============================================================================
// Lexer - Token Types
// ============================================================================

/// Token types produced by the lexer
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Raw text content
    Text(String),
    /// Output with escaping: <%= ... %>
    OutputEscaped(String),
    /// Output without escaping: <%- ... %>  
    OutputRaw(String),
    /// Code block: <% ... %>
    Code(String),
    /// Comment: <%# ... %>
    Comment(String),
}

/// Lexer for EJS templates - uses character-based iteration to handle Unicode
pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
        }
    }

    /// Tokenize the entire input
    pub fn tokenize(&mut self) -> Result<Vec<Token>, EjsError> {
        let mut tokens = Vec::new();

        while self.pos < self.chars.len() {
            if self.starts_with("<%") {
                // EJS tag
                let token = self.read_ejs_tag()?;
                tokens.push(token);
            } else {
                // Text content
                let text = self.read_text();
                if !text.is_empty() {
                    tokens.push(Token::Text(text));
                }
            }
        }

        Ok(tokens)
    }

    fn starts_with(&self, s: &str) -> bool {
        let s_chars: Vec<char> = s.chars().collect();
        if self.pos + s_chars.len() > self.chars.len() {
            return false;
        }
        for (i, c) in s_chars.iter().enumerate() {
            if self.chars[self.pos + i] != *c {
                return false;
            }
        }
        true
    }

    fn read_text(&mut self) -> String {
        let mut result = String::new();
        while self.pos < self.chars.len() && !self.starts_with("<%") {
            let c = self.chars[self.pos];
            if c == '\n' {
                self.line += 1;
            }
            result.push(c);
            self.pos += 1;
        }
        result
    }

    fn read_ejs_tag(&mut self) -> Result<Token, EjsError> {
        let start_line = self.line;
        self.pos += 2; // Skip <%

        if self.pos >= self.chars.len() {
            return Err(EjsError::ParseError {
                line: start_line,
                message: "Unexpected end of template after <%".to_string(),
            });
        }

        // Determine tag type
        let (tag_type, skip) = match self.current_char() {
            '=' => ("escaped", 1),
            '-' => ("raw", 1),
            '#' => ("comment", 1),
            '_' => ("code", 1), // trim start
            _ => ("code", 0),
        };
        self.pos += skip;

        // Find closing %>
        let content_start = self.pos;
        let mut found_close = false;

        while self.pos < self.chars.len() {
            if self.current_char() == '\n' {
                self.line += 1;
            }
            if self.starts_with("%>") {
                found_close = true;
                break;
            }
            self.pos += 1;
        }

        if !found_close {
            return Err(EjsError::ParseError {
                line: start_line,
                message: "Unclosed EJS tag".to_string(),
            });
        }

        let mut content_end = self.pos;
        // Handle trim markers before %>
        if content_end > content_start {
            let last_char = self.chars[content_end - 1];
            if last_char == '-' || last_char == '_' {
                content_end -= 1;
            }
        }

        let content: String = self.chars[content_start..content_end].iter().collect();
        let content = content.trim().to_string();
        self.pos += 2; // Skip %>

        let token = match tag_type {
            "escaped" => Token::OutputEscaped(content),
            "raw" => Token::OutputRaw(content),
            "comment" => Token::Comment(content),
            _ => Token::Code(content),
        };

        Ok(token)
    }

    fn current_char(&self) -> char {
        if self.pos < self.chars.len() {
            self.chars[self.pos]
        } else {
            '\0'
        }
    }
}

// ============================================================================
// AST - Abstract Syntax Tree
// ============================================================================

/// AST node types
#[derive(Debug, Clone)]
pub enum AstNode {
    /// Raw text to output
    Text(String),

    /// Output expression with HTML escaping
    OutputEscaped(String),

    /// Output expression without escaping
    OutputRaw(String),

    /// If statement with optional else-if and else branches
    If {
        condition: String,
        then_branch: Vec<AstNode>,
        else_if_branches: Vec<(String, Vec<AstNode>)>,
        else_branch: Option<Vec<AstNode>>,
    },

    /// Each/forEach loop
    Each {
        array_expr: String,
        item_var: String,
        index_var: Option<String>,
        body: Vec<AstNode>,
    },

    /// For...of loop
    ForOf {
        item_var: String,
        iterable: String,
        body: Vec<AstNode>,
    },

    /// For...in loop (iterate over object keys)
    ForIn {
        key_var: String,
        object_expr: String,
        body: Vec<AstNode>,
    },

    /// Variable declaration
    VarDecl { name: String, value: String },

    /// Comment (ignored in output)
    Comment(String),

    /// Generic code that doesn't fit other patterns
    Code(String),

    /// Sequence of nodes (for multi-statement code blocks)
    Sequence(Vec<AstNode>),
}

// ============================================================================
// Parser - Tokens to AST
// ============================================================================

/// Parser that converts tokens to AST
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse tokens into AST nodes
    pub fn parse(&mut self) -> Result<Vec<AstNode>, EjsError> {
        self.parse_nodes(&[])
    }

    /// Parse nodes until we hit a terminator or end
    fn parse_nodes(&mut self, terminators: &[&str]) -> Result<Vec<AstNode>, EjsError> {
        let mut nodes = Vec::new();

        while self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];

            // Check for terminators
            if let Token::Code(code) = token {
                let code_trimmed = code.trim();
                for term in terminators {
                    if code_trimmed == *term || code_trimmed.starts_with(term) {
                        return Ok(nodes);
                    }
                }
            }

            let node = self.parse_node()?;
            if let Some(n) = node {
                nodes.push(n);
            }
        }

        Ok(nodes)
    }

    /// Parse a single node
    fn parse_node(&mut self) -> Result<Option<AstNode>, EjsError> {
        if self.pos >= self.tokens.len() {
            return Ok(None);
        }

        let token = self.tokens[self.pos].clone();
        self.pos += 1;

        match token {
            Token::Text(text) => Ok(Some(AstNode::Text(text))),
            Token::OutputEscaped(expr) => Ok(Some(AstNode::OutputEscaped(expr))),
            Token::OutputRaw(expr) => Ok(Some(AstNode::OutputRaw(expr))),
            Token::Comment(text) => Ok(Some(AstNode::Comment(text))),
            Token::Code(code) => self.parse_code(&code),
        }
    }

    /// Parse a code block and potentially consume following tokens for control structures
    fn parse_code(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        let code = code.trim();

        // Check if this is a self-contained multi-statement code block
        // This happens when an entire code block including if-else is in one <% ... %>
        if self.is_self_contained_block(code) {
            return self.parse_inline_code_block(code);
        }

        // Handle if statement
        if code.starts_with("if") && (code.contains("(") || code.contains("{")) {
            return self.parse_if_statement(code);
        }

        // Handle each/forEach loop
        if code.contains(".each(") || code.contains(".forEach(") {
            return self.parse_each_loop(code);
        }

        // Handle for...of loop
        if code.starts_with("for") && code.contains(" of ") {
            return self.parse_for_of_loop(code);
        }

        // Handle for...in loop
        if code.starts_with("for") && code.contains(" in ") {
            return self.parse_for_in_loop(code);
        }

        // Handle variable declaration
        if code.starts_with("var ") || code.starts_with("let ") || code.starts_with("const ") {
            return self.parse_var_decl(code);
        }

        // Handle closing braces (should be consumed by control structures)
        if code == "}" || code == "})" || code.starts_with("})") {
            return Ok(None);
        }

        // Handle else/else if (should be consumed by if parsing)
        if code.starts_with("} else") {
            return Ok(None);
        }

        // Generic code
        Ok(Some(AstNode::Code(code.to_string())))
    }

    /// Check if a code block is self-contained (has balanced braces)
    fn is_self_contained_block(&self, code: &str) -> bool {
        // A self-contained block has:
        // 1. Multiple lines with statements
        // 2. Contains an if statement with its body
        // 3. All braces are balanced
        if !code.contains('\n') {
            return false;
        }

        // Check if it contains a complete if statement
        if !code.contains("if") || !code.contains('{') {
            return false;
        }

        // Count braces to see if balanced
        let open_count = code.chars().filter(|c| *c == '{').count();
        let close_count = code.chars().filter(|c| *c == '}').count();

        // If braces are balanced and we have at least one pair, it's self-contained
        open_count > 0 && open_count == close_count
    }

    /// Parse a self-contained inline code block (multi-statement JavaScript)
    fn parse_inline_code_block(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        let mut nodes = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = code.chars().collect();

        while pos < chars.len() {
            // Skip whitespace
            while pos < chars.len() && chars[pos].is_whitespace() {
                pos += 1;
            }
            if pos >= chars.len() {
                break;
            }

            // Get the current statement start
            let stmt_start = pos;

            // Check what kind of statement this is
            let remaining = &code[pos..];

            // Variable declaration
            if remaining.starts_with("var ")
                || remaining.starts_with("let ")
                || remaining.starts_with("const ")
            {
                // Find the end of the statement (semicolon or newline before if/else/for)
                let end_pos = self.find_statement_end(remaining);
                let stmt = remaining[..end_pos].trim();

                // Parse var declaration
                let rest = stmt
                    .strip_prefix("const ")
                    .or_else(|| stmt.strip_prefix("let "))
                    .or_else(|| stmt.strip_prefix("var "))
                    .unwrap_or(stmt);
                let rest = rest.trim().trim_end_matches(';');

                if let Some(eq_pos) = rest.find('=') {
                    let name = rest[..eq_pos].trim().to_string();
                    let value = rest[eq_pos + 1..].trim().to_string();
                    nodes.push(AstNode::VarDecl { name, value });
                }
                pos += end_pos;
                continue;
            }

            // If statement
            if remaining.starts_with("if ") || remaining.starts_with("if(") {
                if let Some(if_node) = self.parse_inline_if_statement(remaining)? {
                    nodes.push(if_node);
                }
                // The entire remaining code is consumed by the if statement
                break;
            }

            // Assignment or other statements
            if let Some(semi_pos) = remaining.find(';') {
                let stmt = remaining[..semi_pos].trim();
                if !stmt.is_empty() {
                    nodes.push(AstNode::Code(stmt.to_string()));
                }
                pos += semi_pos + 1;
                continue;
            }

            // Move forward to avoid infinite loop
            pos = stmt_start + 1;
        }

        if nodes.is_empty() {
            Ok(None)
        } else if nodes.len() == 1 {
            Ok(Some(nodes.into_iter().next().unwrap()))
        } else {
            Ok(Some(AstNode::Sequence(nodes)))
        }
    }

    /// Find the end of a simple statement (semicolon or start of control structure)
    fn find_statement_end(&self, code: &str) -> usize {
        let mut pos = 0;
        let chars: Vec<char> = code.chars().collect();

        while pos < chars.len() {
            if chars[pos] == ';' {
                return pos + 1;
            }
            if chars[pos] == '\n' {
                // Check if next non-whitespace is a control structure
                let remaining = &code[pos + 1..];
                let trimmed = remaining.trim_start();
                if trimmed.starts_with("if ")
                    || trimmed.starts_with("if(")
                    || trimmed.starts_with("else")
                    || trimmed.starts_with("for ")
                    || trimmed.starts_with("while ")
                {
                    return pos;
                }
            }
            pos += 1;
        }

        code.len()
    }

    /// Parse a self-contained if statement (with body inline)
    fn parse_inline_if_statement(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        let code = code.trim();

        // Extract condition
        let condition = extract_condition(code);
        if condition.is_empty() {
            return Ok(Some(AstNode::Code(code.to_string())));
        }

        // Find the opening brace of the if body
        let open_brace_pos = match code.find('{') {
            Some(p) => p,
            None => return Ok(Some(AstNode::Code(code.to_string()))),
        };

        // Find the matching closing brace
        let (then_end, then_body) = self.extract_brace_block(&code[open_brace_pos..])?;

        // Parse the then branch body
        let then_nodes = self.parse_inline_statements(&then_body)?;

        // Check for else-if and else branches
        let mut else_if_branches = Vec::new();
        let mut else_branch = None;

        let mut remaining = &code[open_brace_pos + then_end..];
        remaining = remaining.trim();

        while !remaining.is_empty() {
            if remaining.starts_with("else if ")
                || remaining.starts_with("else if(")
                || remaining.starts_with("} else if ")
                || remaining.starts_with("} else if(")
            {
                // Skip the "} else if" part if present
                let skip = if remaining.starts_with("} ") { 2 } else { 0 };
                remaining = &remaining[skip..];

                // Extract else-if condition
                let else_if_cond = extract_condition(remaining);

                // Find and extract the body
                if let Some(brace_pos) = remaining.find('{') {
                    let (body_end, body) = self.extract_brace_block(&remaining[brace_pos..])?;
                    let body_nodes = self.parse_inline_statements(&body)?;
                    else_if_branches.push((else_if_cond, body_nodes));
                    remaining = &remaining[brace_pos + body_end..];
                    remaining = remaining.trim();
                } else {
                    break;
                }
            } else if remaining.starts_with("else {")
                || remaining.starts_with("else{")
                || remaining.starts_with("} else {")
                || remaining.starts_with("} else{")
            {
                // Skip the "} else {" part
                let brace_pos = remaining.find('{').unwrap();
                let (_, body) = self.extract_brace_block(&remaining[brace_pos..])?;
                let body_nodes = self.parse_inline_statements(&body)?;
                else_branch = Some(body_nodes);
                break; // else is always last
            } else {
                break;
            }
        }

        Ok(Some(AstNode::If {
            condition,
            then_branch: then_nodes,
            else_if_branches,
            else_branch,
        }))
    }

    /// Extract a brace-delimited block, returning (end_position, inner_content)
    fn extract_brace_block(&self, code: &str) -> Result<(usize, String), EjsError> {
        let chars: Vec<char> = code.chars().collect();

        if chars.is_empty() || chars[0] != '{' {
            return Err(EjsError::ParseError {
                line: 0,
                message: "Expected opening brace".to_string(),
            });
        }

        let mut depth = 0;
        let mut in_string = false;
        let mut string_char = ' ';

        for (i, &c) in chars.iter().enumerate() {
            if in_string {
                if c == string_char && (i == 0 || chars[i - 1] != '\\') {
                    in_string = false;
                }
                continue;
            }

            match c {
                '"' | '\'' | '`' => {
                    in_string = true;
                    string_char = c;
                }
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        // Return the content between braces (excluding the braces)
                        return Ok((i + 1, code[1..i].to_string()));
                    }
                }
                _ => {}
            }
        }

        Err(EjsError::ParseError {
            line: 0,
            message: "Unbalanced braces".to_string(),
        })
    }

    /// Parse statements from inside a block body
    fn parse_inline_statements(&mut self, body: &str) -> Result<Vec<AstNode>, EjsError> {
        let mut nodes = Vec::new();
        let body = body.trim();

        if body.is_empty() {
            return Ok(nodes);
        }

        // Split by lines and process each statement
        let mut pos = 0;
        let chars: Vec<char> = body.chars().collect();

        while pos < chars.len() {
            // Skip whitespace
            while pos < chars.len() && chars[pos].is_whitespace() {
                pos += 1;
            }
            if pos >= chars.len() {
                break;
            }

            let remaining = &body[pos..];

            // Nested if statement
            if remaining.starts_with("if ") || remaining.starts_with("if(") {
                if let Some(if_node) = self.parse_inline_if_statement(remaining)? {
                    nodes.push(if_node);
                }
                // Skip the entire if block
                break; // For now, assume if takes the rest
            }

            // Variable assignment or other statement - find semicolon or newline
            let end = remaining
                .find(';')
                .map(|p| p + 1)
                .unwrap_or_else(|| remaining.find('\n').unwrap_or(remaining.len()));

            let stmt = remaining[..end].trim().trim_end_matches(';');
            if !stmt.is_empty() && !stmt.starts_with("//") {
                // Check if it's an assignment like "title = __('archive_a')"
                nodes.push(AstNode::Code(stmt.to_string()));
            }

            pos += end;
        }

        Ok(nodes)
    }

    /// Parse an if statement with optional else-if and else branches
    fn parse_if_statement(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        let condition = extract_condition(code);

        // Parse then branch
        let then_branch = self.parse_nodes(&["}", "} else {", "} else if"])?;

        let mut else_if_branches = Vec::new();
        let mut else_branch = None;

        // Check for else-if and else branches
        while self.pos < self.tokens.len() {
            if let Token::Code(next_code) = &self.tokens[self.pos] {
                let next_code = next_code.trim();

                if next_code.starts_with("} else if") {
                    self.pos += 1;
                    let else_if_condition = extract_condition(next_code);
                    let else_if_body = self.parse_nodes(&["}", "} else {", "} else if"])?;
                    else_if_branches.push((else_if_condition, else_if_body));
                } else if next_code == "} else {" {
                    self.pos += 1;
                    else_branch = Some(self.parse_nodes(&["}"])?);
                    // Consume closing brace
                    if self.pos < self.tokens.len() {
                        if let Token::Code(c) = &self.tokens[self.pos] {
                            if c.trim() == "}" {
                                self.pos += 1;
                            }
                        }
                    }
                    break;
                } else if next_code == "}" {
                    self.pos += 1;
                    break;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(Some(AstNode::If {
            condition,
            then_branch,
            else_if_branches,
            else_branch,
        }))
    }

    /// Parse an each/forEach loop
    fn parse_each_loop(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        // Pattern: array.each(function(item, index){  or  array.forEach(function(item){
        let (array_expr, rest) = if let Some(pos) = code.find(".each(") {
            (&code[..pos], &code[pos + 6..])
        } else if let Some(pos) = code.find(".forEach(") {
            (&code[..pos], &code[pos + 9..])
        } else {
            return Ok(Some(AstNode::Code(code.to_string())));
        };

        // Extract function parameters
        let rest = rest.trim();

        // Handle both traditional function and arrow function syntax
        // Arrow function: post => { or (post, index) => {
        // Traditional: function(post) { or function(post, index) {
        let (item_var, index_var) = if rest.contains("=>") {
            // Arrow function syntax
            let arrow_pos = rest.find("=>").unwrap();
            let params_part = rest[..arrow_pos].trim();

            // Remove parentheses if present
            let params_str = params_part.trim_start_matches('(').trim_end_matches(')');
            let params: Vec<&str> = params_str.split(',').map(|s| s.trim()).collect();

            if params.is_empty() || params[0].is_empty() {
                return Ok(Some(AstNode::Code(code.to_string())));
            }

            (params[0].to_string(), params.get(1).map(|s| s.to_string()))
        } else if rest.starts_with("function(") || rest.starts_with("function (") {
            // Traditional function syntax
            let func_start = rest.find('(').unwrap() + 1;
            let func_end = rest.find(')').unwrap_or(rest.len());
            let params_str = &rest[func_start..func_end];
            let params: Vec<&str> = params_str.split(',').map(|s| s.trim()).collect();

            if params.is_empty() {
                return Ok(Some(AstNode::Code(code.to_string())));
            }

            (params[0].to_string(), params.get(1).map(|s| s.to_string()))
        } else {
            return Ok(Some(AstNode::Code(code.to_string())));
        };

        // Parse loop body
        let body = self.parse_nodes(&["})", "});"])?;

        // Consume closing })
        if self.pos < self.tokens.len() {
            if let Token::Code(c) = &self.tokens[self.pos] {
                let c = c.trim();
                if c == "})" || c.starts_with("})") || c.ends_with("})") {
                    self.pos += 1;
                }
            }
        }

        Ok(Some(AstNode::Each {
            array_expr: array_expr.to_string(),
            item_var,
            index_var,
            body,
        }))
    }

    /// Parse a for...of loop
    fn parse_for_of_loop(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        if let Some((var_name, iterable)) = parse_for_of_loop(code) {
            let body = self.parse_nodes(&["}"])?;

            // Consume closing brace
            if self.pos < self.tokens.len() {
                if let Token::Code(c) = &self.tokens[self.pos] {
                    if c.trim() == "}" {
                        self.pos += 1;
                    }
                }
            }

            Ok(Some(AstNode::ForOf {
                item_var: var_name,
                iterable,
                body,
            }))
        } else {
            Ok(Some(AstNode::Code(code.to_string())))
        }
    }

    /// Parse a for...in loop
    fn parse_for_in_loop(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        if let Some((key_var, object_expr)) = parse_for_in_loop(code) {
            let body = self.parse_nodes(&["}"])?;

            // Consume closing brace
            if self.pos < self.tokens.len() {
                if let Token::Code(c) = &self.tokens[self.pos] {
                    if c.trim() == "}" {
                        self.pos += 1;
                    }
                }
            }

            Ok(Some(AstNode::ForIn {
                key_var,
                object_expr,
                body,
            }))
        } else {
            Ok(Some(AstNode::Code(code.to_string())))
        }
    }

    /// Parse a variable declaration
    fn parse_var_decl(&mut self, code: &str) -> Result<Option<AstNode>, EjsError> {
        let rest = code
            .strip_prefix("const ")
            .or_else(|| code.strip_prefix("let "))
            .or_else(|| code.strip_prefix("var "))
            .unwrap_or(code);
        let rest = rest.trim().trim_end_matches(';');

        if let Some(eq_pos) = rest.find('=') {
            let name = rest[..eq_pos].trim().to_string();
            let value = rest[eq_pos + 1..].trim().to_string();
            Ok(Some(AstNode::VarDecl { name, value }))
        } else {
            Ok(Some(AstNode::Code(code.to_string())))
        }
    }
}

// ============================================================================
// EjsValue - Runtime values
// ============================================================================

/// A value in the EJS context
#[derive(Debug, Clone)]
pub enum EjsValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<EjsValue>),
    Object(IndexMap<String, EjsValue>),
}

impl EjsValue {
    /// Convert to string for output
    pub fn to_output_string(&self) -> String {
        match self {
            EjsValue::Null => "".to_string(),
            EjsValue::Bool(b) => b.to_string(),
            EjsValue::Number(n) => {
                if n.fract() == 0.0 {
                    (*n as i64).to_string()
                } else {
                    n.to_string()
                }
            }
            EjsValue::String(s) => s.clone(),
            EjsValue::Array(arr) => arr
                .iter()
                .map(|v| v.to_output_string())
                .collect::<Vec<_>>()
                .join(","),
            EjsValue::Object(_) => "[object Object]".to_string(),
        }
    }

    /// Check if the value is truthy
    pub fn is_truthy(&self) -> bool {
        match self {
            EjsValue::Null => false,
            EjsValue::Bool(b) => *b,
            EjsValue::Number(n) => *n != 0.0,
            EjsValue::String(s) => !s.is_empty(),
            EjsValue::Array(arr) => !arr.is_empty(),
            EjsValue::Object(obj) => !obj.is_empty(),
        }
    }

    /// Get a property from an object
    pub fn get_property(&self, key: &str) -> Option<&EjsValue> {
        match self {
            EjsValue::Object(obj) => obj.get(key),
            EjsValue::Array(arr) => {
                if let Ok(idx) = key.parse::<usize>() {
                    arr.get(idx)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get array length
    pub fn len(&self) -> usize {
        match self {
            EjsValue::Array(arr) => arr.len(),
            EjsValue::String(s) => s.len(),
            EjsValue::Object(obj) => obj.len(),
            _ => 0,
        }
    }

    /// Check if value is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Convert from serde_json::Value
    pub fn from_json(json: &serde_json::Value) -> Self {
        match json {
            serde_json::Value::Null => EjsValue::Null,
            serde_json::Value::Bool(b) => EjsValue::Bool(*b),
            serde_json::Value::Number(n) => EjsValue::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => EjsValue::String(s.clone()),
            serde_json::Value::Array(arr) => {
                EjsValue::Array(arr.iter().map(EjsValue::from_json).collect())
            }
            serde_json::Value::Object(obj) => {
                let mut map = IndexMap::new();
                for (k, v) in obj {
                    map.insert(k.clone(), EjsValue::from_json(v));
                }
                EjsValue::Object(map)
            }
        }
    }

    /// Compare two values for equality
    pub fn equals(&self, other: &EjsValue) -> bool {
        match (self, other) {
            (EjsValue::Null, EjsValue::Null) => true,
            (EjsValue::Bool(a), EjsValue::Bool(b)) => a == b,
            (EjsValue::Number(a), EjsValue::Number(b)) => (a - b).abs() < f64::EPSILON,
            (EjsValue::String(a), EjsValue::String(b)) => a == b,
            _ => false,
        }
    }

    /// Convert to serde_json::Value
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            EjsValue::Null => serde_json::Value::Null,
            EjsValue::Bool(b) => serde_json::Value::Bool(*b),
            EjsValue::Number(n) => serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            EjsValue::String(s) => serde_json::Value::String(s.clone()),
            EjsValue::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| v.to_json()).collect())
            }
            EjsValue::Object(obj) => {
                let map: serde_json::Map<String, serde_json::Value> =
                    obj.iter().map(|(k, v)| (k.clone(), v.to_json())).collect();
                serde_json::Value::Object(map)
            }
        }
    }
}

impl From<&str> for EjsValue {
    fn from(s: &str) -> Self {
        EjsValue::String(s.to_string())
    }
}

impl From<String> for EjsValue {
    fn from(s: String) -> Self {
        EjsValue::String(s)
    }
}

impl From<bool> for EjsValue {
    fn from(b: bool) -> Self {
        EjsValue::Bool(b)
    }
}

impl From<i32> for EjsValue {
    fn from(n: i32) -> Self {
        EjsValue::Number(n as f64)
    }
}

impl From<i64> for EjsValue {
    fn from(n: i64) -> Self {
        EjsValue::Number(n as f64)
    }
}

impl From<f64> for EjsValue {
    fn from(n: f64) -> Self {
        EjsValue::Number(n)
    }
}

impl From<usize> for EjsValue {
    fn from(n: usize) -> Self {
        EjsValue::Number(n as f64)
    }
}

// ============================================================================
// EjsContext - Template context
// ============================================================================

/// Rendering context for EJS templates
#[derive(Debug, Clone, Default)]
pub struct EjsContext {
    variables: HashMap<String, EjsValue>,
}

impl EjsContext {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    pub fn set(&mut self, name: &str, value: EjsValue) {
        self.variables.insert(name.to_string(), value);
    }

    /// Set a nested property using dot notation (e.g., "page.posts")
    pub fn set_nested(&mut self, path: &str, value: EjsValue) {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() == 1 {
            self.set(path, value);
            return;
        }

        let root_name = parts[0];
        let root = self
            .variables
            .entry(root_name.to_string())
            .or_insert_with(|| EjsValue::Object(IndexMap::new()));

        let mut current = root;
        for &part in &parts[1..parts.len() - 1] {
            current = match current {
                EjsValue::Object(ref mut obj) => obj
                    .entry(part.to_string())
                    .or_insert_with(|| EjsValue::Object(IndexMap::new())),
                _ => return,
            };
        }

        if let EjsValue::Object(ref mut obj) = current {
            obj.insert(parts[parts.len() - 1].to_string(), value);
        }
    }

    /// Set a nested object property using dot notation
    pub fn set_nested_object<T: serde::Serialize>(&mut self, path: &str, value: &T) {
        if let Ok(json) = serde_json::to_value(value) {
            self.set_nested(path, EjsValue::from_json(&json));
        }
    }

    pub fn get(&self, name: &str) -> Option<&EjsValue> {
        self.variables.get(name)
    }

    pub fn set_string(&mut self, name: &str, value: &str) {
        self.set(name, EjsValue::String(value.to_string()));
    }

    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set(name, EjsValue::Bool(value));
    }

    pub fn set_number(&mut self, name: &str, value: f64) {
        self.set(name, EjsValue::Number(value));
    }

    pub fn set_object<T: serde::Serialize>(&mut self, name: &str, value: &T) {
        if let Ok(json) = serde_json::to_value(value) {
            self.set(name, EjsValue::from_json(&json));
        }
    }

    pub fn variables(&self) -> &HashMap<String, EjsValue> {
        &self.variables
    }

    pub fn merge(&mut self, other: &EjsContext) {
        for (k, v) in &other.variables {
            self.variables.insert(k.clone(), v.clone());
        }
    }

    /// Convert context to JSON string
    pub fn to_json(&self) -> String {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.variables {
            map.insert(k.clone(), v.to_json());
        }
        serde_json::Value::Object(map).to_string()
    }
}

// ============================================================================
// Evaluator - AST to output
// ============================================================================

/// Evaluator that renders AST nodes to output
pub struct Evaluator<'a> {
    context: &'a EjsContext,
    local_vars: HashMap<String, EjsValue>,
    partials: Option<Arc<HashMap<String, String>>>,
}

impl<'a> Evaluator<'a> {
    pub fn new(context: &'a EjsContext, partials: Option<Arc<HashMap<String, String>>>) -> Self {
        Self {
            context,
            local_vars: HashMap::new(),
            partials,
        }
    }

    /// Render AST nodes to string
    pub fn render(&mut self, nodes: &[AstNode]) -> Result<String, EjsError> {
        let mut output = String::new();
        for node in nodes {
            self.render_node(node, &mut output)?;
        }
        Ok(output)
    }

    fn render_node(&mut self, node: &AstNode, output: &mut String) -> Result<(), EjsError> {
        match node {
            AstNode::Text(text) => {
                output.push_str(text);
            }
            AstNode::OutputEscaped(expr) => {
                let value = self.evaluate_expr(expr)?;
                output.push_str(&html_escape(&value));
            }
            AstNode::OutputRaw(expr) => {
                let value = self.evaluate_expr(expr)?;
                output.push_str(&value);
            }
            AstNode::If {
                condition,
                then_branch,
                else_if_branches,
                else_branch,
            } => {
                let cond_value = self.evaluate_to_value(condition)?;
                if cond_value.is_truthy() {
                    for n in then_branch {
                        self.render_node(n, output)?;
                    }
                } else {
                    let mut executed = false;
                    for (else_if_cond, else_if_body) in else_if_branches {
                        let else_if_value = self.evaluate_to_value(else_if_cond)?;
                        if else_if_value.is_truthy() {
                            for n in else_if_body {
                                self.render_node(n, output)?;
                            }
                            executed = true;
                            break;
                        }
                    }
                    if !executed {
                        if let Some(else_body) = else_branch {
                            for n in else_body {
                                self.render_node(n, output)?;
                            }
                        }
                    }
                }
            }
            AstNode::Each {
                array_expr,
                item_var,
                index_var,
                body,
            } => {
                let array_value = self.evaluate_to_value(array_expr)?;
                if let EjsValue::Array(items) = array_value {
                    for (idx, item) in items.iter().enumerate() {
                        self.local_vars.insert(item_var.clone(), item.clone());
                        if let Some(idx_var) = index_var {
                            self.local_vars
                                .insert(idx_var.clone(), EjsValue::Number(idx as f64));
                        }
                        for n in body {
                            self.render_node(n, output)?;
                        }
                    }
                    self.local_vars.remove(item_var);
                    if let Some(idx_var) = index_var {
                        self.local_vars.remove(idx_var);
                    }
                }
            }
            AstNode::ForOf {
                item_var,
                iterable,
                body,
            } => {
                let iter_value = self.evaluate_to_value(iterable)?;
                if let EjsValue::Array(items) = iter_value {
                    for item in items.iter() {
                        self.local_vars.insert(item_var.clone(), item.clone());
                        for n in body {
                            self.render_node(n, output)?;
                        }
                    }
                    self.local_vars.remove(item_var);
                }
            }
            AstNode::ForIn {
                key_var,
                object_expr,
                body,
            } => {
                let obj_value = self.evaluate_to_value(object_expr)?;
                if let EjsValue::Object(obj) = obj_value {
                    for key in obj.keys() {
                        self.local_vars
                            .insert(key_var.clone(), EjsValue::String(key.clone()));
                        for n in body {
                            self.render_node(n, output)?;
                        }
                    }
                    self.local_vars.remove(key_var);
                }
            }
            AstNode::VarDecl { name, value } => {
                let val = self.evaluate_to_value(value)?;
                self.local_vars.insert(name.clone(), val);
            }
            AstNode::Comment(_) => {
                // Comments are ignored
            }
            AstNode::Code(code) => {
                // Handle assignments like "title = __('archive_a')" or "title += ': ' + page.year"
                let code = code.trim().trim_end_matches(';');

                // Check for += operator (compound assignment)
                if let Some(eq_pos) = code.find("+=") {
                    let var_name = code[..eq_pos].trim();
                    let value_expr = code[eq_pos + 2..].trim();

                    // Get current value
                    let current = self
                        .local_vars
                        .get(var_name)
                        .cloned()
                        .unwrap_or(EjsValue::String(String::new()));

                    // Evaluate right-hand side
                    if let Ok(rhs_value) = self.evaluate_to_value(value_expr) {
                        // Concatenate strings
                        let new_value = format!(
                            "{}{}",
                            current.to_output_string(),
                            rhs_value.to_output_string()
                        );
                        self.local_vars
                            .insert(var_name.to_string(), EjsValue::String(new_value));
                    }
                } else if let Some(eq_pos) = code.find('=') {
                    // Simple assignment: title = value
                    // But make sure it's not == or != or <= or >=
                    let before_eq = if eq_pos > 0 {
                        code.chars().nth(eq_pos - 1)
                    } else {
                        None
                    };
                    let after_eq = code.chars().nth(eq_pos + 1);

                    if before_eq != Some('!')
                        && before_eq != Some('=')
                        && before_eq != Some('<')
                        && before_eq != Some('>')
                        && after_eq != Some('=')
                    {
                        let var_name = code[..eq_pos].trim();
                        let value_expr = code[eq_pos + 1..].trim();

                        // Only process if var_name is a simple identifier
                        if var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            if let Ok(value) = self.evaluate_to_value(value_expr) {
                                // If we have a pre-set value and the new value is 0/empty/array,
                                // keep the pre-set value (for complex JS expressions that can't be evaluated)
                                let has_existing = self.local_vars.contains_key(var_name)
                                    || self.context.get(var_name).is_some();

                                let is_fallback_value = match &value {
                                    EjsValue::Number(n) if *n == 0.0 => true,
                                    EjsValue::String(s) if s.is_empty() => true,
                                    EjsValue::Null => true,
                                    EjsValue::Array(arr) if arr.is_empty() => true,
                                    _ => false,
                                };

                                if has_existing && is_fallback_value {
                                    // Keep the existing value, don't overwrite with fallback
                                    tracing::debug!(
                                        "Keeping existing value for {} instead of {:?}",
                                        var_name,
                                        value
                                    );
                                } else {
                                    self.local_vars.insert(var_name.to_string(), value);
                                }
                            }
                        }
                    }
                } else if code.contains("(") && code.ends_with(")") {
                    // Function call that outputs directly
                    if let Ok(result) = self.evaluate_expr(code) {
                        output.push_str(&result);
                    }
                }
            }
            AstNode::Sequence(nodes) => {
                for n in nodes {
                    self.render_node(n, output)?;
                }
            }
        }
        Ok(())
    }

    /// Evaluate an expression to string
    fn evaluate_expr(&self, expr: &str) -> Result<String, EjsError> {
        let value = self.evaluate_to_value(expr)?;
        Ok(value.to_output_string())
    }

    /// Evaluate an expression to EjsValue
    fn evaluate_to_value(&self, expr: &str) -> Result<EjsValue, EjsError> {
        let expr = expr.trim();

        if expr.is_empty() {
            return Ok(EjsValue::Null);
        }

        // String literal
        if let Some(s) = try_parse_string_literal(expr) {
            return Ok(EjsValue::String(s));
        }

        // Number literal
        if let Ok(n) = expr.parse::<f64>() {
            return Ok(EjsValue::Number(n));
        }

        // Boolean literals
        if expr == "true" {
            return Ok(EjsValue::Bool(true));
        }
        if expr == "false" {
            return Ok(EjsValue::Bool(false));
        }
        if expr == "null" || expr == "undefined" {
            return Ok(EjsValue::Null);
        }

        // Parenthesized expression - unwrap and evaluate the inner expression
        // This handles cases like (a && b) or (x == y)
        if expr.starts_with('(') && expr.ends_with(')') {
            // Check if this is a complete parenthesized expression
            // (not a function call like func() or nested like (a)(b))
            let inner = &expr[1..expr.len() - 1];
            // Verify the parentheses are balanced
            let mut depth = 0;
            let mut valid = true;
            for c in inner.chars() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth < 0 {
                            valid = false;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if valid && depth == 0 {
                return self.evaluate_to_value(inner);
            }
        }

        // Array literal
        if expr.starts_with('[') && expr.ends_with(']') {
            let inner = &expr[1..expr.len() - 1];
            let elements = parse_function_args(inner);
            let values: Result<Vec<EjsValue>, EjsError> = elements
                .iter()
                .map(|e| self.evaluate_to_value(e.trim()))
                .collect();
            return Ok(EjsValue::Array(values?));
        }

        // Empty object literal {}
        if expr == "{}" {
            return Ok(EjsValue::Object(IndexMap::new()));
        }

        // Logical operators (lowest precedence - check first!)
        // These must be checked before function calls to properly handle complex conditions
        // like: url_for(x) == '/' || is_home()

        // Logical OR
        if let Some(pos) = find_operator(expr, "||") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            if left.is_truthy() {
                return Ok(left);
            }
            return self.evaluate_to_value(expr[pos + 2..].trim());
        }

        // Logical AND
        if let Some(pos) = find_operator(expr, "&&") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            if !left.is_truthy() {
                return Ok(left);
            }
            return self.evaluate_to_value(expr[pos + 2..].trim());
        }

        // Ternary operator (before function calls)
        if let Some((condition, rest)) = split_ternary(expr) {
            if let Some((true_val, false_val)) = rest.split_once(':') {
                let cond_result = self.evaluate_to_value(condition.trim())?;
                return if cond_result.is_truthy() {
                    self.evaluate_to_value(true_val.trim())
                } else {
                    self.evaluate_to_value(false_val.trim())
                };
            }
        }

        // Comparison operators (before function calls)
        if let Some(result) = self.try_comparison(expr)? {
            return Ok(result);
        }

        // Addition or string concatenation with + operator
        // Find + that's not inside quotes or parentheses
        if let Some(pos) = find_plus_operator(expr) {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            let right = self.evaluate_to_value(expr[pos + 1..].trim())?;

            // If both operands are numbers, do numeric addition
            if let (EjsValue::Number(l), EjsValue::Number(r)) = (&left, &right) {
                return Ok(EjsValue::Number(l + r));
            }

            // Otherwise do string concatenation
            let result = format!("{}{}", left.to_output_string(), right.to_output_string());
            return Ok(EjsValue::String(result));
        }

        // partial() function
        if expr.starts_with("partial(") && expr.ends_with(')') {
            return self.evaluate_partial(expr);
        }

        // Other function calls
        // Handle moment() chained calls like moment(Date.now()).tz('Asia/Shanghai').locale('zh-cn').format('YYYY-MM-DD, a h:mm')
        if expr.starts_with("moment(") && expr.contains(".format(") {
            return self.evaluate_moment_chain(expr);
        }

        // Handle chained method calls like new Date().getFullYear()
        if let Some(paren_pos) = expr.find('(') {
            if expr.ends_with(')') {
                // Find the matching closing paren for the first open paren
                // to properly handle chained calls like new Date().getFullYear()
                let first_close = find_matching_paren(expr, paren_pos);

                if first_close == expr.len() - 1 {
                    // Simple function call: func(args)
                    let func_name = &expr[..paren_pos];
                    let args_str = &expr[paren_pos + 1..expr.len() - 1];
                    return self.evaluate_function_call(func_name, args_str);
                } else if first_close < expr.len() - 1 {
                    // Chained call: base().method() or base().method(args)
                    // Find the last method call
                    let rest = &expr[first_close + 1..];
                    if let Some(method_part) = rest.strip_prefix('.') {
                        // We have a method call on the result
                        // skip the '.'
                        if let Some(method_paren) = method_part.find('(') {
                            let method_name = &method_part[..method_paren];
                            let base_expr = &expr[..first_close + 1];

                            // Special handling for Date methods
                            if base_expr == "new Date()" {
                                match method_name {
                                    "getFullYear" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(now.year() as f64));
                                    }
                                    "getMonth" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(now.month0() as f64));
                                    }
                                    "getDate" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(now.day() as f64));
                                    }
                                    "getDay" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(
                                            now.weekday().num_days_from_sunday() as f64,
                                        ));
                                    }
                                    "getHours" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(now.hour() as f64));
                                    }
                                    "getMinutes" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(now.minute() as f64));
                                    }
                                    "getSeconds" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(now.second() as f64));
                                    }
                                    "getTime" => {
                                        let now = chrono::Utc::now();
                                        return Ok(EjsValue::Number(now.timestamp_millis() as f64));
                                    }
                                    _ => {}
                                }
                            }

                            // For other chained calls, evaluate base and call method
                            let base_value = self.evaluate_to_value(base_expr)?;
                            let method_args = &method_part[method_paren + 1..method_part.len() - 1];
                            let args = parse_function_args(method_args);
                            return self.evaluate_array_method(&base_value, method_name, &args);
                        }
                    }
                }

                // Fallback: simple function call
                let func_name = &expr[..paren_pos];
                let args_str = &expr[paren_pos + 1..expr.len() - 1];
                return self.evaluate_function_call(func_name, args_str);
            }
        }

        // Negation
        if let Some(rest) = expr.strip_prefix('!') {
            let inner = self.evaluate_to_value(rest.trim())?;
            return Ok(EjsValue::Bool(!inner.is_truthy()));
        }

        // Variable/property access
        self.resolve_variable(expr)
    }

    fn try_comparison(&self, expr: &str) -> Result<Option<EjsValue>, EjsError> {
        // !==
        if let Some(pos) = find_operator(expr, "!==") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            let right = self.evaluate_to_value(expr[pos + 3..].trim())?;
            return Ok(Some(EjsValue::Bool(!left.equals(&right))));
        }
        // ===
        if let Some(pos) = find_operator(expr, "===") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            let right = self.evaluate_to_value(expr[pos + 3..].trim())?;
            return Ok(Some(EjsValue::Bool(left.equals(&right))));
        }
        // !=
        if let Some(pos) = find_operator(expr, "!=") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            let right = self.evaluate_to_value(expr[pos + 2..].trim())?;
            return Ok(Some(EjsValue::Bool(!left.equals(&right))));
        }
        // ==
        if let Some(pos) = find_operator(expr, "==") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            let right = self.evaluate_to_value(expr[pos + 2..].trim())?;
            return Ok(Some(EjsValue::Bool(left.equals(&right))));
        }
        // >=
        if let Some(pos) = find_operator(expr, ">=") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            let right = self.evaluate_to_value(expr[pos + 2..].trim())?;
            return Ok(Some(EjsValue::Bool(compare_values(&left, &right) >= 0)));
        }
        // <=
        if let Some(pos) = find_operator(expr, "<=") {
            let left = self.evaluate_to_value(expr[..pos].trim())?;
            let right = self.evaluate_to_value(expr[pos + 2..].trim())?;
            return Ok(Some(EjsValue::Bool(compare_values(&left, &right) <= 0)));
        }
        // >
        if let Some(pos) = find_operator(expr, ">") {
            if !expr[pos..].starts_with(">=") {
                let left = self.evaluate_to_value(expr[..pos].trim())?;
                let right = self.evaluate_to_value(expr[pos + 1..].trim())?;
                return Ok(Some(EjsValue::Bool(compare_values(&left, &right) > 0)));
            }
        }
        // <
        if let Some(pos) = find_operator(expr, "<") {
            if !expr[pos..].starts_with("<=") {
                let left = self.evaluate_to_value(expr[..pos].trim())?;
                let right = self.evaluate_to_value(expr[pos + 1..].trim())?;
                return Ok(Some(EjsValue::Bool(compare_values(&left, &right) < 0)));
            }
        }
        Ok(None)
    }

    fn resolve_variable(&self, path: &str) -> Result<EjsValue, EjsError> {
        let path = path.trim();

        // Bracket access
        if let Some(bracket_pos) = path.find('[') {
            let base = &path[..bracket_pos];
            let rest = &path[bracket_pos..];
            let base_val = self.resolve_variable(base)?;
            return self.resolve_bracket_access(&base_val, rest);
        }

        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Ok(EjsValue::Null);
        }

        let root = parts[0];
        let mut current = if let Some(val) = self.local_vars.get(root) {
            val.clone()
        } else if let Some(val) = self.context.get(root) {
            val.clone()
        } else {
            return Ok(EjsValue::Null);
        };

        for part in &parts[1..] {
            if *part == "length" {
                return Ok(EjsValue::Number(current.len() as f64));
            }
            if let Some(val) = current.get_property(part) {
                current = val.clone();
            } else {
                return Ok(EjsValue::Null);
            }
        }

        Ok(current)
    }

    fn resolve_bracket_access(&self, base: &EjsValue, access: &str) -> Result<EjsValue, EjsError> {
        let mut current = base.clone();
        let mut remaining = access;

        while remaining.starts_with('[') {
            let close_pos = remaining.find(']').unwrap_or(remaining.len());
            let key_expr = &remaining[1..close_pos];
            let key_expr = key_expr.trim();

            // Evaluate the key expression - could be a string literal or a variable
            let key = if key_expr.starts_with('"') || key_expr.starts_with('\'') {
                // String literal - just strip quotes
                key_expr.trim_matches('"').trim_matches('\'').to_string()
            } else if let Ok(n) = key_expr.parse::<usize>() {
                // Numeric index
                n.to_string()
            } else {
                // Variable reference - evaluate it
                self.evaluate_to_value(key_expr)?.to_output_string()
            };

            if let Some(val) = current.get_property(&key) {
                current = val.clone();
            } else {
                return Ok(EjsValue::Null);
            }

            remaining = &remaining[close_pos + 1..];

            if remaining.starts_with('.') {
                remaining = &remaining[1..];
                if let Some(dot_pos) = remaining.find(['[', '.']) {
                    let part = &remaining[..dot_pos];
                    if let Some(val) = current.get_property(part) {
                        current = val.clone();
                    } else {
                        return Ok(EjsValue::Null);
                    }
                    remaining = &remaining[dot_pos..];
                } else if !remaining.is_empty() {
                    if let Some(val) = current.get_property(remaining) {
                        current = val.clone();
                    } else {
                        return Ok(EjsValue::Null);
                    }
                    break;
                }
            }
        }

        Ok(current)
    }

    fn evaluate_partial(&self, expr: &str) -> Result<EjsValue, EjsError> {
        let args_str = &expr[8..expr.len() - 1];
        let args = parse_function_args(args_str);

        if args.is_empty() {
            return Ok(EjsValue::String(String::new()));
        }

        // Evaluate the partial name as an expression (handles string concatenation like '_widget/' + widget)
        let partial_name = self.evaluate_to_value(&args[0])?.to_output_string();

        let locals = if args.len() > 1 {
            self.parse_object_literal(&args[1])?
        } else {
            IndexMap::new()
        };

        if let Some(partials) = &self.partials {
            let names_to_try = vec![
                partial_name.clone(),
                format!("_partial/{}", partial_name),
                format!("partial/{}", partial_name),
                format!("_widget/{}", partial_name),
                format!("widget/{}", partial_name),
                partial_name.replace("_partial/", ""),
                partial_name.replace("_widget/", ""),
                format!("post/{}", partial_name),
                format!("_partial/post/{}", partial_name),
            ];

            for name in &names_to_try {
                if let Some(partial_content) = partials.get(name) {
                    let engine = EjsEngine::new();
                    if let Ok(template) = engine.parse(partial_content) {
                        let mut partial_context = self.context.clone();
                        for (k, v) in &self.local_vars {
                            partial_context.set(k, v.clone());
                        }
                        for (k, v) in locals.iter() {
                            partial_context.set(k, v.clone());
                        }

                        let mut sub_eval = Evaluator::new(&partial_context, self.partials.clone());
                        if let Ok(result) = sub_eval.render(&template.ast) {
                            return Ok(EjsValue::String(result));
                        }
                    }
                }
            }
            tracing::debug!(
                "Partial not found: {} (tried: {:?})",
                partial_name,
                names_to_try
            );
        }

        Ok(EjsValue::String(String::new()))
    }

    fn parse_object_literal(&self, expr: &str) -> Result<IndexMap<String, EjsValue>, EjsError> {
        let mut result = IndexMap::new();
        let expr = expr.trim();

        if !expr.starts_with('{') || !expr.ends_with('}') {
            if let Ok(EjsValue::Object(obj)) = self.evaluate_to_value(expr) {
                return Ok(obj);
            }
            return Ok(result);
        }

        let inner = &expr[1..expr.len() - 1];
        let pairs = parse_object_pairs(inner);

        for (key, value_expr) in pairs {
            let value = self.evaluate_to_value(&value_expr)?;
            result.insert(key, value);
        }

        Ok(result)
    }

    /// Get a config value from the context
    fn get_config_value(&self, key: &str) -> Option<String> {
        if let Some(EjsValue::Object(config)) = self.context.get("config") {
            if let Some(val) = config.get(key) {
                return Some(val.to_output_string());
            }
        }
        None
    }

    /// Get site categories from context
    fn get_site_categories(&self) -> HashMap<String, usize> {
        let mut result = HashMap::new();
        if let Some(EjsValue::Object(site)) = self.context.get("site") {
            if let Some(EjsValue::Object(categories)) = site.get("categories") {
                for (name, count) in categories {
                    if let EjsValue::Number(n) = count {
                        result.insert(name.clone(), *n as usize);
                    }
                }
            }
        }
        result
    }

    /// Get site tags from context
    fn get_site_tags(&self) -> HashMap<String, usize> {
        let mut result = HashMap::new();
        if let Some(EjsValue::Object(site)) = self.context.get("site") {
            if let Some(EjsValue::Object(tags)) = site.get("tags") {
                for (name, count) in tags {
                    if let EjsValue::Number(n) = count {
                        result.insert(name.clone(), *n as usize);
                    }
                }
            }
        }
        result
    }

    /// Get site posts from context
    fn get_site_posts(&self) -> Vec<IndexMap<String, EjsValue>> {
        let mut result = Vec::new();
        if let Some(EjsValue::Object(site)) = self.context.get("site") {
            if let Some(EjsValue::Array(posts)) = site.get("posts") {
                for post in posts {
                    if let EjsValue::Object(post_obj) = post {
                        result.push(post_obj.clone());
                    }
                }
            }
        }
        result
    }

    /// Evaluate a function call
    fn evaluate_function_call(
        &self,
        func_name: &str,
        args_str: &str,
    ) -> Result<EjsValue, EjsError> {
        let args = parse_function_args(args_str);

        match func_name.trim() {
            "partial" => self.evaluate_partial(&format!("partial({})", args_str)),

            "url_for" => {
                let root = self
                    .get_config_value("root")
                    .unwrap_or_else(|| "/".to_string());
                let root = root.trim_end_matches('/');

                if let Some(first_arg) = args.first() {
                    let path = self.evaluate_to_value(first_arg)?.to_output_string();
                    let path = path.trim_start_matches('/');
                    let url = if path.is_empty() {
                        format!("{}/", root)
                    } else {
                        format!("{}/{}", root, path)
                    };
                    Ok(EjsValue::String(url))
                } else {
                    // url_for() without arguments returns root
                    Ok(EjsValue::String(format!("{}/", root)))
                }
            }

            "css" => {
                if let Some(first_arg) = args.first() {
                    let value = self.evaluate_to_value(first_arg)?;
                    let root = self
                        .get_config_value("root")
                        .unwrap_or_else(|| "/".to_string());
                    let root = root.trim_end_matches('/');

                    // Helper to generate a single link tag
                    let generate_link = |path: String, root: &str| -> String {
                        let href = if path.starts_with("http://")
                            || path.starts_with("https://")
                            || path.starts_with("//")
                        {
                            path
                        } else {
                            let path = if path.ends_with(".css") {
                                path
                            } else {
                                format!("{}.css", path)
                            };
                            format!("{}/{}", root, path.trim_start_matches('/'))
                        };
                        format!(r#"<link rel="stylesheet" href="{}">"#, href)
                    };

                    // Handle array or single value
                    match value {
                        EjsValue::Array(items) => {
                            let links: Vec<String> = items
                                .iter()
                                .map(|item| generate_link(item.to_output_string(), root))
                                .collect();
                            Ok(EjsValue::String(links.join("\n")))
                        }
                        _ => {
                            let path = value.to_output_string();
                            Ok(EjsValue::String(generate_link(path, root)))
                        }
                    }
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "js" => {
                if let Some(first_arg) = args.first() {
                    let value = self.evaluate_to_value(first_arg)?;
                    let root = self
                        .get_config_value("root")
                        .unwrap_or_else(|| "/".to_string());
                    let root = root.trim_end_matches('/');

                    // Helper to generate a single script tag
                    let generate_script = |path: String, root: &str| -> String {
                        let src = if path.starts_with("http://")
                            || path.starts_with("https://")
                            || path.starts_with("//")
                        {
                            path
                        } else {
                            let path = if path.ends_with(".js") {
                                path
                            } else {
                                format!("{}.js", path)
                            };
                            format!("{}/{}", root, path.trim_start_matches('/'))
                        };
                        format!(r#"<script src="{}"></script>"#, src)
                    };

                    // Handle array or single value
                    match value {
                        EjsValue::Array(items) => {
                            let scripts: Vec<String> = items
                                .iter()
                                .map(|item| generate_script(item.to_output_string(), root))
                                .collect();
                            Ok(EjsValue::String(scripts.join("\n")))
                        }
                        _ => {
                            let path = value.to_output_string();
                            Ok(EjsValue::String(generate_script(path, root)))
                        }
                    }
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "__" => {
                if let Some(first_arg) = args.first() {
                    let key = self.evaluate_to_value(first_arg)?.to_output_string();
                    if let Some(EjsValue::Object(translations)) = self.context.get("__") {
                        if let Some(val) = translations.get(&key) {
                            return Ok(val.clone());
                        }
                    }
                    Ok(EjsValue::String(key))
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "date" => {
                if args.len() >= 2 {
                    let date_val = self.evaluate_to_value(&args[0])?;
                    let format_val = self.evaluate_to_value(&args[1])?;
                    let format = if format_val.is_truthy() {
                        format_val.to_output_string()
                    } else {
                        self.get_config_value("date_format")
                            .unwrap_or_else(|| "YYYY-MM-DD".to_string())
                    };
                    let date_str = date_val.to_output_string();
                    let formatted = parse_and_format_date(&date_str, &format);
                    Ok(EjsValue::String(formatted))
                } else if args.len() == 1 {
                    let date_val = self.evaluate_to_value(&args[0])?;
                    let date_str = date_val.to_output_string();
                    let format = self
                        .get_config_value("date_format")
                        .unwrap_or_else(|| "YYYY-MM-DD".to_string());
                    let formatted = parse_and_format_date(&date_str, &format);
                    Ok(EjsValue::String(formatted))
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "date_xml" => {
                if let Some(first_arg) = args.first() {
                    let date_val = self.evaluate_to_value(first_arg)?;
                    let date_str = date_val.to_output_string();
                    let formatted = parse_and_format_date_xml(&date_str);
                    Ok(EjsValue::String(formatted))
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "time_tag" | "timeTag" => {
                if let Some(first_arg) = args.first() {
                    let date_val = self.evaluate_to_value(first_arg)?;
                    let date_str = date_val.to_output_string();
                    let format = if args.len() >= 2 {
                        self.evaluate_to_value(&args[1])?.to_output_string()
                    } else {
                        "YYYY-MM-DD".to_string()
                    };
                    let datetime = parse_and_format_date_xml(&date_str);
                    let display = parse_and_format_date(&date_str, &format);
                    Ok(EjsValue::String(format!(
                        r#"<time datetime="{}">{}</time>"#,
                        datetime, display
                    )))
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "paginator" => {
                let page = self.context.get("page");
                if let Some(EjsValue::Object(page_obj)) = page {
                    let current = page_obj
                        .get("current")
                        .map(|v| match v {
                            EjsValue::Number(n) => *n as usize,
                            _ => 1,
                        })
                        .unwrap_or(1);
                    let total = page_obj
                        .get("total")
                        .map(|v| match v {
                            EjsValue::Number(n) => *n as usize,
                            _ => 1,
                        })
                        .unwrap_or(1);

                    let root = self
                        .get_config_value("root")
                        .unwrap_or_else(|| "/".to_string());
                    let mut html = String::new();

                    if current > 1 {
                        let prev_link = if current == 2 {
                            root.clone()
                        } else {
                            format!("{}page/{}/", root.trim_end_matches('/'), current - 1)
                        };
                        html.push_str(&format!(
                            r#"<a class="prev" href="{}">&laquo; Prev</a>"#,
                            prev_link
                        ));
                    }

                    for i in 1..=total {
                        if i == current {
                            html.push_str(&format!(
                                r#"<span class="page-number current">{}</span>"#,
                                i
                            ));
                        } else {
                            let link = if i == 1 {
                                root.clone()
                            } else {
                                format!("{}page/{}/", root.trim_end_matches('/'), i)
                            };
                            html.push_str(&format!(
                                r#"<a class="page-number" href="{}">{}</a>"#,
                                link, i
                            ));
                        }
                    }

                    if current < total {
                        let next_link =
                            format!("{}page/{}/", root.trim_end_matches('/'), current + 1);
                        html.push_str(&format!(
                            r#"<a class="next" href="{}">Next &raquo;</a>"#,
                            next_link
                        ));
                    }

                    Ok(EjsValue::String(html))
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "strip_html" | "stripHTML" => {
                if let Some(first_arg) = args.first() {
                    let content = self.evaluate_to_value(first_arg)?.to_output_string();
                    let stripped = strip_html_tags(&content);
                    Ok(EjsValue::String(stripped))
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "truncate" => {
                if args.len() >= 2 {
                    let content = self.evaluate_to_value(&args[0])?.to_output_string();
                    let length = self
                        .evaluate_to_value(&args[1])?
                        .to_output_string()
                        .parse::<usize>()
                        .unwrap_or(100);
                    let truncated = if content.chars().count() > length {
                        let s: String = content.chars().take(length).collect();
                        format!("{}...", s.trim_end())
                    } else {
                        content
                    };
                    Ok(EjsValue::String(truncated))
                } else {
                    Ok(EjsValue::String(String::new()))
                }
            }

            "list_categories" => {
                // Get options from args
                let options = if let Some(arg) = args.first() {
                    self.evaluate_to_value(arg)?
                } else {
                    EjsValue::Object(IndexMap::new())
                };

                let show_count = match &options {
                    EjsValue::Object(obj) => {
                        obj.get("show_count").map(|v| v.is_truthy()).unwrap_or(true)
                    }
                    _ => true,
                };

                // Get categories from site.categories
                let categories = self.get_site_categories();
                let root = self
                    .get_config_value("root")
                    .unwrap_or_else(|| "/".to_string());
                let category_dir = self
                    .get_config_value("category_dir")
                    .unwrap_or_else(|| "categories".to_string());

                if categories.is_empty() {
                    return Ok(EjsValue::String(String::new()));
                }

                let mut html = String::from(r#"<ul class="category-list">"#);
                let mut sorted: Vec<_> = categories.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));

                for (name, count) in sorted {
                    let slug = slug::slugify(name);
                    let url = format!("{}{}/{}/", root.trim_end_matches('/'), category_dir, slug);

                    html.push_str(&format!(
                        r#"<li class="category-list-item"><a class="category-list-link" href="{}">{}</a>"#,
                        url, name
                    ));

                    if show_count {
                        html.push_str(&format!(
                            r#"<span class="category-list-count">{}</span>"#,
                            count
                        ));
                    }
                    html.push_str("</li>");
                }
                html.push_str("</ul>");
                Ok(EjsValue::String(html))
            }

            "list_tags" => {
                let options = if let Some(arg) = args.first() {
                    self.evaluate_to_value(arg)?
                } else {
                    EjsValue::Object(IndexMap::new())
                };

                let show_count = match &options {
                    EjsValue::Object(obj) => {
                        obj.get("show_count").map(|v| v.is_truthy()).unwrap_or(true)
                    }
                    _ => true,
                };

                let tags = self.get_site_tags();
                let root = self
                    .get_config_value("root")
                    .unwrap_or_else(|| "/".to_string());
                let tag_dir = self
                    .get_config_value("tag_dir")
                    .unwrap_or_else(|| "tags".to_string());

                if tags.is_empty() {
                    return Ok(EjsValue::String(String::new()));
                }

                let mut html = String::from(r#"<ul class="tag-list">"#);
                let mut sorted: Vec<_> = tags.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));

                for (name, count) in sorted {
                    let slug = slug::slugify(name);
                    let url = format!("{}{}/{}/", root.trim_end_matches('/'), tag_dir, slug);

                    html.push_str(&format!(
                        r#"<li class="tag-list-item"><a class="tag-list-link" href="{}">{}</a>"#,
                        url, name
                    ));

                    if show_count {
                        html.push_str(&format!(r#"<span class="tag-list-count">{}</span>"#, count));
                    }
                    html.push_str("</li>");
                }
                html.push_str("</ul>");
                Ok(EjsValue::String(html))
            }

            "list_archives" => {
                let options = if let Some(arg) = args.first() {
                    self.evaluate_to_value(arg)?
                } else {
                    EjsValue::Object(IndexMap::new())
                };

                let show_count = match &options {
                    EjsValue::Object(obj) => {
                        obj.get("show_count").map(|v| v.is_truthy()).unwrap_or(true)
                    }
                    _ => true,
                };

                let archive_type = match &options {
                    EjsValue::Object(obj) => obj
                        .get("type")
                        .map(|v| v.to_output_string())
                        .unwrap_or_else(|| "monthly".to_string()),
                    _ => "monthly".to_string(),
                };

                let posts = self.get_site_posts();
                let root = self
                    .get_config_value("root")
                    .unwrap_or_else(|| "/".to_string());
                let archive_dir = self
                    .get_config_value("archive_dir")
                    .unwrap_or_else(|| "archives".to_string());

                // Group posts by year or year/month
                let mut archives: HashMap<String, usize> = HashMap::new();
                for post in &posts {
                    let date = post
                        .get("date")
                        .map(|v| v.to_output_string())
                        .unwrap_or_default();
                    // Parse date (format: YYYY-MM-DD or similar)
                    let key = if archive_type == "yearly" {
                        date.get(..4).unwrap_or("").to_string()
                    } else {
                        date.get(..7)
                            .map(|s| s.replace('-', "/"))
                            .unwrap_or_default()
                    };
                    if !key.is_empty() {
                        *archives.entry(key).or_insert(0) += 1;
                    }
                }

                if archives.is_empty() {
                    return Ok(EjsValue::String(String::new()));
                }

                let mut html = String::from(r#"<ul class="archive-list">"#);
                let mut sorted: Vec<_> = archives.iter().collect();
                sorted.sort_by(|a, b| b.0.cmp(a.0)); // Descending

                for (key, count) in sorted {
                    let url = format!("{}{}/{}/", root.trim_end_matches('/'), archive_dir, key);

                    let display = if archive_type == "yearly" {
                        key.clone()
                    } else {
                        // Convert YYYY/MM to "Month Year"
                        let parts: Vec<&str> = key.split('/').collect();
                        if parts.len() == 2 {
                            let month_name = match parts[1] {
                                "01" => "January",
                                "02" => "February",
                                "03" => "March",
                                "04" => "April",
                                "05" => "May",
                                "06" => "June",
                                "07" => "July",
                                "08" => "August",
                                "09" => "September",
                                "10" => "October",
                                "11" => "November",
                                "12" => "December",
                                _ => parts[1],
                            };
                            format!("{} {}", month_name, parts[0])
                        } else {
                            key.clone()
                        }
                    };

                    html.push_str(&format!(
                        r#"<li class="archive-list-item"><a class="archive-list-link" href="{}">{}</a>"#,
                        url, display
                    ));

                    if show_count {
                        html.push_str(&format!(
                            r#"<span class="archive-list-count">{}</span>"#,
                            count
                        ));
                    }
                    html.push_str("</li>");
                }
                html.push_str("</ul>");
                Ok(EjsValue::String(html))
            }

            "tagcloud" => {
                let tags = self.get_site_tags();
                let root = self
                    .get_config_value("root")
                    .unwrap_or_else(|| "/".to_string());
                let tag_dir = self
                    .get_config_value("tag_dir")
                    .unwrap_or_else(|| "tags".to_string());

                if tags.is_empty() {
                    return Ok(EjsValue::String(String::new()));
                }

                let min_count = *tags.values().min().unwrap_or(&1) as f32;
                let max_count = *tags.values().max().unwrap_or(&1) as f32;
                let count_range = (max_count - min_count).max(1.0);

                let min_font = 10.0_f32;
                let max_font = 20.0_f32;

                let mut html = String::from(r#"<div class="tagcloud">"#);
                let mut sorted: Vec<_> = tags.iter().collect();
                sorted.sort_by(|a, b| a.0.cmp(b.0));

                for (name, count) in sorted {
                    let slug = slug::slugify(name);
                    let url = format!("{}{}/{}/", root.trim_end_matches('/'), tag_dir, slug);

                    let size = if count_range > 0.0 {
                        let ratio = (*count as f32 - min_count) / count_range;
                        min_font + ratio * (max_font - min_font)
                    } else {
                        min_font
                    };

                    html.push_str(&format!(
                        r#"<a href="{}" style="font-size: {:.2}px;">{}</a> "#,
                        url, size, name
                    ));
                }
                html.push_str("</div>");
                Ok(EjsValue::String(html))
            }

            "search_form" => {
                let options = if let Some(arg) = args.first() {
                    self.evaluate_to_value(arg)?
                } else {
                    EjsValue::Object(IndexMap::new())
                };

                let button = match &options {
                    EjsValue::Object(obj) => obj
                        .get("button")
                        .map(|v| v.to_output_string())
                        .unwrap_or_else(|| "Search".to_string()),
                    _ => "Search".to_string(),
                };

                let text = match &options {
                    EjsValue::Object(obj) => obj
                        .get("text")
                        .map(|v| v.to_output_string())
                        .unwrap_or_else(|| "Search".to_string()),
                    _ => "Search".to_string(),
                };

                let html = format!(
                    r#"<form action="//google.com/search" method="get" accept-charset="UTF-8" class="search-form"><input type="search" name="q" class="search-form-input" placeholder="{}"><button type="submit" class="search-form-submit">{}</button><input type="hidden" name="sitesearch" value=""></form>"#,
                    text, button
                );
                Ok(EjsValue::String(html))
            }

            "feed_tag" => {
                let rss_path = if let Some(arg) = args.first() {
                    let val = self.evaluate_to_value(arg)?;
                    if val.is_truthy() {
                        val.to_output_string()
                    } else {
                        "atom.xml".to_string()
                    }
                } else {
                    "atom.xml".to_string()
                };

                let root = self
                    .get_config_value("root")
                    .unwrap_or_else(|| "/".to_string());
                let url = format!("{}{}", root.trim_end_matches('/'), rss_path);

                let html = format!(
                    r#"<link rel="alternate" href="{}" title="" type="application/atom+xml">"#,
                    url
                );
                Ok(EjsValue::String(html))
            }

            "open_graph" => {
                // Basic Open Graph implementation
                let options = if let Some(arg) = args.first() {
                    self.evaluate_to_value(arg)?
                } else {
                    EjsValue::Object(IndexMap::new())
                };

                let mut html = String::new();

                // Get page data
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    let title = page
                        .get("title")
                        .map(|v| v.to_output_string())
                        .unwrap_or_default();
                    let description = page
                        .get("description")
                        .or_else(|| page.get("excerpt"))
                        .map(|v| strip_html_tags(&v.to_output_string()))
                        .unwrap_or_default();
                    let url = page
                        .get("permalink")
                        .map(|v| v.to_output_string())
                        .unwrap_or_default();

                    if !title.is_empty() {
                        html.push_str(&format!(
                            r#"<meta property="og:title" content="{}">"#,
                            html_escape(&title)
                        ));
                    }
                    if !description.is_empty() {
                        let desc: String = description.chars().take(200).collect();
                        html.push_str(&format!(
                            r#"<meta property="og:description" content="{}">"#,
                            html_escape(&desc)
                        ));
                    }
                    if !url.is_empty() {
                        html.push_str(&format!(r#"<meta property="og:url" content="{}">"#, url));
                    }
                }

                html.push_str(r#"<meta property="og:type" content="website">"#);

                // Add Twitter card if twitter_id provided
                if let EjsValue::Object(opts) = &options {
                    if let Some(twitter_id) = opts.get("twitter_id") {
                        let id = twitter_id.to_output_string();
                        if !id.is_empty() {
                            html.push_str(r#"<meta name="twitter:card" content="summary">"#);
                            html.push_str(&format!(
                                r#"<meta name="twitter:site" content="@{}">"#,
                                id.trim_start_matches('@')
                            ));
                        }
                    }
                }

                Ok(EjsValue::String(html))
            }

            "favicon_tag" => {
                // Generate favicon link tag: <%- favicon_tag(path) %>
                let favicon_path = if let Some(arg) = args.first() {
                    let val = self.evaluate_to_value(arg)?;
                    if val.is_truthy() {
                        val.to_output_string()
                    } else {
                        // If falsy (undefined, null, false), return empty string (no favicon)
                        return Ok(EjsValue::String(String::new()));
                    }
                } else {
                    // Default favicon path
                    "/favicon.ico".to_string()
                };

                let root = self
                    .get_config_value("root")
                    .unwrap_or_else(|| "/".to_string());

                let href = if favicon_path.starts_with("http://")
                    || favicon_path.starts_with("https://")
                    || favicon_path.starts_with("//")
                {
                    favicon_path
                } else {
                    format!(
                        "{}{}",
                        root.trim_end_matches('/'),
                        if favicon_path.starts_with('/') {
                            favicon_path
                        } else {
                            format!("/{}", favicon_path)
                        }
                    )
                };

                let html = format!(r#"<link rel="icon" href="{}">"#, href);
                Ok(EjsValue::String(html))
            }

            // Page type helper functions
            "is_home" => {
                // Check if current page is home/index page
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    // If page has is_home explicitly set, use that
                    if let Some(is_home_val) = page.get("is_home") {
                        return Ok(EjsValue::Bool(is_home_val.is_truthy()));
                    }

                    let is_archive = page
                        .get("is_archive")
                        .map(|v| v.is_truthy())
                        .unwrap_or(false);
                    let is_category = page
                        .get("is_category")
                        .map(|v| v.is_truthy())
                        .unwrap_or(false);
                    let is_tag = page.get("is_tag").map(|v| v.is_truthy()).unwrap_or(false);

                    // Check if this is a regular page (has a layout that's not index)
                    let layout = page
                        .get("layout")
                        .map(|v| v.to_output_string())
                        .unwrap_or_default();
                    // Any non-empty layout that's not "index" means it's a specific page/post
                    let has_specific_layout = !layout.is_empty() && layout != "index";

                    let current = page
                        .get("current")
                        .map(|v| match v {
                            EjsValue::Number(n) => *n as usize,
                            _ => 1,
                        })
                        .unwrap_or(1);
                    // Home is when it's not archive/category/tag, has no specific layout, and is first page
                    let is_home = !is_archive
                        && !is_category
                        && !is_tag
                        && !has_specific_layout
                        && current == 1;
                    Ok(EjsValue::Bool(is_home))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "is_post" => {
                // Check if current page is a single post
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    // A post page has layout = "post" or has "content" but no "posts" array
                    let layout = page
                        .get("layout")
                        .map(|v| v.to_output_string())
                        .unwrap_or_default();
                    let has_content = page.get("content").map(|v| v.is_truthy()).unwrap_or(false);
                    let has_posts = page
                        .get("posts")
                        .map(|v| matches!(v, EjsValue::Array(_)))
                        .unwrap_or(false);
                    let is_post = layout == "post" || (has_content && !has_posts);
                    Ok(EjsValue::Bool(is_post))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "is_page" => {
                // Check if current page is a standalone page (not post, not list)
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    let layout = page
                        .get("layout")
                        .map(|v| v.to_output_string())
                        .unwrap_or_default();
                    Ok(EjsValue::Bool(layout == "page"))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "is_archive" => {
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    let is_archive = page
                        .get("is_archive")
                        .map(|v| v.is_truthy())
                        .unwrap_or(false);
                    tracing::debug!(
                        "is_archive() called, page.is_archive = {:?}, result = {}",
                        page.get("is_archive"),
                        is_archive
                    );
                    Ok(EjsValue::Bool(is_archive))
                } else {
                    tracing::debug!("is_archive() called but page context not found");
                    Ok(EjsValue::Bool(false))
                }
            }

            "is_category" => {
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    let is_category = page
                        .get("is_category")
                        .map(|v| v.is_truthy())
                        .unwrap_or(false);
                    Ok(EjsValue::Bool(is_category))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "is_tag" => {
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    let is_tag = page.get("is_tag").map(|v| v.is_truthy()).unwrap_or(false);
                    Ok(EjsValue::Bool(is_tag))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "is_year" => {
                // Check if it's a yearly archive page
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    let is_archive = page
                        .get("is_archive")
                        .map(|v| v.is_truthy())
                        .unwrap_or(false);
                    let has_year = page.get("year").map(|v| v.is_truthy()).unwrap_or(false);
                    let has_month = page.get("month").map(|v| v.is_truthy()).unwrap_or(false);
                    Ok(EjsValue::Bool(is_archive && has_year && !has_month))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "is_month" => {
                // Check if it's a monthly archive page
                if let Some(EjsValue::Object(page)) = self.context.get("page") {
                    let is_archive = page
                        .get("is_archive")
                        .map(|v| v.is_truthy())
                        .unwrap_or(false);
                    let has_year = page.get("year").map(|v| v.is_truthy()).unwrap_or(false);
                    let has_month = page.get("month").map(|v| v.is_truthy()).unwrap_or(false);
                    Ok(EjsValue::Bool(is_archive && has_year && has_month))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            _ => {
                // Try to handle method chaining on arrays, e.g., site.posts.sort('date', -1).limit(5)
                // Check if this is a method call on an object (contains . before method name)

                // Handle new Date() constructor
                if func_name == "new Date" {
                    let now = chrono::Utc::now();
                    return Ok(EjsValue::String(
                        now.format("%Y-%m-%d %H:%M:%S").to_string(),
                    ));
                }

                // Handle new Date().getFullYear(), new Date().getMonth(), etc.
                if let Some(method) = func_name.strip_prefix("new Date().") {
                    let now = chrono::Utc::now();
                    match method {
                        "getFullYear" => return Ok(EjsValue::Number(now.year() as f64)),
                        "getMonth" => return Ok(EjsValue::Number((now.month0()) as f64)), // JS months are 0-indexed
                        "getDate" => return Ok(EjsValue::Number(now.day() as f64)),
                        "getDay" => {
                            return Ok(
                                EjsValue::Number(now.weekday().num_days_from_sunday() as f64),
                            )
                        }
                        "getHours" => return Ok(EjsValue::Number(now.hour() as f64)),
                        "getMinutes" => return Ok(EjsValue::Number(now.minute() as f64)),
                        "getSeconds" => return Ok(EjsValue::Number(now.second() as f64)),
                        "getTime" => return Ok(EjsValue::Number(now.timestamp_millis() as f64)),
                        _ => {}
                    }
                }

                // Check for chained method calls ending in .sort, .limit, etc.
                // Pattern: object.method1(...).method2(...)
                if let Some(last_dot) = func_name.rfind('.') {
                    let base_expr = &func_name[..last_dot];
                    let method_name = &func_name[last_dot + 1..];

                    // Try to evaluate the base expression first
                    // Check if base_expr itself contains method calls
                    if base_expr.contains('(') {
                        // This is a chained method call like site.posts.sort('date', -1).limit
                        // We need to evaluate site.posts.sort('date', -1) first
                        if let Ok(base_value) = self.evaluate_to_value(base_expr) {
                            return self.evaluate_array_method(&base_value, method_name, &args);
                        }
                    } else {
                        // Simple property access followed by method call like site.posts.sort
                        if let Ok(base_value) = self.resolve_variable(base_expr) {
                            return self.evaluate_array_method(&base_value, method_name, &args);
                        }
                    }
                }

                tracing::debug!("Unknown function: {}", func_name);
                Ok(EjsValue::String(String::new()))
            }
        }
    }

    /// Evaluate array method calls like .sort(), .limit()
    fn evaluate_array_method(
        &self,
        base: &EjsValue,
        method: &str,
        args: &[String],
    ) -> Result<EjsValue, EjsError> {
        match method {
            "sort" => {
                if let EjsValue::Array(items) = base {
                    let mut sorted = items.clone();

                    // Get the sort key (first argument, e.g., 'date' or '-date')
                    let sort_key = if let Some(arg) = args.first() {
                        self.evaluate_to_value(arg)?.to_output_string()
                    } else {
                        "date".to_string()
                    };

                    // Get sort direction (second argument, e.g., -1 for descending)
                    let descending = if args.len() >= 2 {
                        matches!(
                            self.evaluate_to_value(&args[1]),
                            Ok(EjsValue::Number(n)) if n < 0.0
                        )
                    } else {
                        // Also check if key starts with '-' for descending
                        sort_key.starts_with('-')
                    };

                    let key = sort_key.trim_start_matches('-');

                    sorted.sort_by(|a, b| {
                        let a_val = a
                            .get_property(key)
                            .map(|v| v.to_output_string())
                            .unwrap_or_default();
                        let b_val = b
                            .get_property(key)
                            .map(|v| v.to_output_string())
                            .unwrap_or_default();

                        let cmp = a_val.cmp(&b_val);
                        if descending {
                            cmp.reverse()
                        } else {
                            cmp
                        }
                    });

                    Ok(EjsValue::Array(sorted))
                } else {
                    Ok(base.clone())
                }
            }

            "limit" => {
                if let EjsValue::Array(items) = base {
                    let limit = if let Some(arg) = args.first() {
                        let val = self.evaluate_to_value(arg)?;
                        match val {
                            EjsValue::Number(n) => n as usize,
                            _ => val.to_output_string().parse().unwrap_or(items.len()),
                        }
                    } else {
                        items.len()
                    };

                    let limited: Vec<EjsValue> = items.iter().take(limit).cloned().collect();
                    Ok(EjsValue::Array(limited))
                } else {
                    Ok(base.clone())
                }
            }

            "slice" => {
                if let EjsValue::Array(items) = base {
                    let start = if let Some(arg) = args.first() {
                        let val = self.evaluate_to_value(arg)?;
                        match val {
                            EjsValue::Number(n) => n as usize,
                            _ => 0,
                        }
                    } else {
                        0
                    };

                    let end = if args.len() >= 2 {
                        let val = self.evaluate_to_value(&args[1])?;
                        match val {
                            EjsValue::Number(n) => n as usize,
                            _ => items.len(),
                        }
                    } else {
                        items.len()
                    };

                    let sliced: Vec<EjsValue> = items
                        .iter()
                        .skip(start)
                        .take(end.saturating_sub(start))
                        .cloned()
                        .collect();
                    Ok(EjsValue::Array(sliced))
                } else {
                    Ok(base.clone())
                }
            }

            "filter" => {
                // Basic filter - for now just return the original array
                // Full filter implementation would need function evaluation
                Ok(base.clone())
            }

            "reverse" => {
                if let EjsValue::Array(items) = base {
                    let reversed: Vec<EjsValue> = items.iter().rev().cloned().collect();
                    Ok(EjsValue::Array(reversed))
                } else {
                    Ok(base.clone())
                }
            }

            "count" | "length" => {
                // Return the count/length of the array
                if let EjsValue::Array(items) = base {
                    Ok(EjsValue::Number(items.len() as f64))
                } else {
                    Ok(EjsValue::Number(0.0))
                }
            }

            "map" => {
                // Map with complex functions can't be fully evaluated
                // Return empty array to avoid [object Object] spam in output
                // Templates should use pre-computed values like site.wordCount instead
                if let EjsValue::Array(_) = base {
                    // If the map function is complex (contains =>), we can't evaluate it
                    // Return an empty array that will produce 0 when reduced
                    if args
                        .iter()
                        .any(|a| a.contains("=>") || a.contains("function"))
                    {
                        return Ok(EjsValue::Array(vec![]));
                    }
                }
                Ok(base.clone())
            }

            "reduce" => {
                // Reduce is complex to implement - return 0 for numeric reductions
                // This is a safe default that won't produce [object Object] in output
                Ok(EjsValue::Number(0.0))
            }

            // String methods
            "replace" => {
                if let EjsValue::String(s) = base {
                    if args.len() >= 2 {
                        let search = self.evaluate_to_value(&args[0])?.to_output_string();
                        let replacement = self.evaluate_to_value(&args[1])?.to_output_string();
                        Ok(EjsValue::String(s.replace(&search, &replacement)))
                    } else {
                        Ok(EjsValue::String(s.clone()))
                    }
                } else {
                    Ok(base.clone())
                }
            }

            "split" => {
                if let EjsValue::String(s) = base {
                    let separator = if let Some(arg) = args.first() {
                        self.evaluate_to_value(arg)?.to_output_string()
                    } else {
                        ",".to_string()
                    };
                    let parts: Vec<EjsValue> = s
                        .split(&separator)
                        .map(|p| EjsValue::String(p.to_string()))
                        .collect();
                    Ok(EjsValue::Array(parts))
                } else {
                    Ok(base.clone())
                }
            }

            "trim" => {
                if let EjsValue::String(s) = base {
                    Ok(EjsValue::String(s.trim().to_string()))
                } else {
                    Ok(base.clone())
                }
            }

            "toLowerCase" => {
                if let EjsValue::String(s) = base {
                    Ok(EjsValue::String(s.to_lowercase()))
                } else {
                    Ok(base.clone())
                }
            }

            "toUpperCase" => {
                if let EjsValue::String(s) = base {
                    Ok(EjsValue::String(s.to_uppercase()))
                } else {
                    Ok(base.clone())
                }
            }

            "substring" | "substr" => {
                if let EjsValue::String(s) = base {
                    let start = if let Some(arg) = args.first() {
                        match self.evaluate_to_value(arg)? {
                            EjsValue::Number(n) => n as usize,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    let end = if args.len() >= 2 {
                        match self.evaluate_to_value(&args[1])? {
                            EjsValue::Number(n) => Some(n as usize),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let result = if let Some(e) = end {
                        s.chars().skip(start).take(e - start).collect()
                    } else {
                        s.chars().skip(start).collect()
                    };
                    Ok(EjsValue::String(result))
                } else {
                    Ok(base.clone())
                }
            }

            "startsWith" => {
                if let EjsValue::String(s) = base {
                    let prefix = if let Some(arg) = args.first() {
                        self.evaluate_to_value(arg)?.to_output_string()
                    } else {
                        return Ok(EjsValue::Bool(false));
                    };
                    Ok(EjsValue::Bool(s.starts_with(&prefix)))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "endsWith" => {
                if let EjsValue::String(s) = base {
                    let suffix = if let Some(arg) = args.first() {
                        self.evaluate_to_value(arg)?.to_output_string()
                    } else {
                        return Ok(EjsValue::Bool(false));
                    };
                    Ok(EjsValue::Bool(s.ends_with(&suffix)))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            "includes" | "indexOf" => {
                if let EjsValue::String(s) = base {
                    let search = if let Some(arg) = args.first() {
                        self.evaluate_to_value(arg)?.to_output_string()
                    } else {
                        return Ok(if method == "includes" {
                            EjsValue::Bool(false)
                        } else {
                            EjsValue::Number(-1.0)
                        });
                    };
                    if method == "includes" {
                        Ok(EjsValue::Bool(s.contains(&search)))
                    } else {
                        let idx = s.find(&search).map(|i| i as f64).unwrap_or(-1.0);
                        Ok(EjsValue::Number(idx))
                    }
                } else if let EjsValue::Array(items) = base {
                    let search = if let Some(arg) = args.first() {
                        self.evaluate_to_value(arg)?
                    } else {
                        return Ok(EjsValue::Bool(false));
                    };
                    Ok(EjsValue::Bool(
                        items.iter().any(|item| item.equals(&search)),
                    ))
                } else {
                    Ok(EjsValue::Bool(false))
                }
            }

            // Date methods - works on date strings like "2025-09-27"
            "year" => {
                let s = base.to_output_string();
                // Try to extract year from date string formats like "2025-09-27" or "2025/09/27"
                if let Some(year_str) = s.split(['-', '/']).next() {
                    if let Ok(year) = year_str.parse::<f64>() {
                        return Ok(EjsValue::Number(year));
                    }
                }
                Ok(EjsValue::Number(0.0))
            }

            "month" => {
                let s = base.to_output_string();
                // Extract month from date string (1-indexed in JS Date, 0-indexed here to match JS)
                let parts: Vec<&str> = s.split(['-', '/']).collect();
                if parts.len() >= 2 {
                    if let Ok(month) = parts[1].parse::<f64>() {
                        return Ok(EjsValue::Number(month - 1.0)); // JS months are 0-indexed
                    }
                }
                Ok(EjsValue::Number(0.0))
            }

            "date" | "day" => {
                let s = base.to_output_string();
                let parts: Vec<&str> = s.split(['-', '/']).collect();
                if parts.len() >= 3 {
                    if let Ok(day) = parts[2].parse::<f64>() {
                        return Ok(EjsValue::Number(day));
                    }
                }
                Ok(EjsValue::Number(0.0))
            }

            _ => {
                tracing::debug!("Unknown array method: {}", method);
                Ok(base.clone())
            }
        }
    }

    /// Evaluate a moment() chained call like moment(Date.now()).tz('...').locale('...').format('...')
    fn evaluate_moment_chain(&self, expr: &str) -> Result<EjsValue, EjsError> {
        // Parse the chain to extract: timestamp, timezone, locale, format
        // Pattern: moment(timestamp).tz('tz').locale('locale').format('format')

        // Extract the timestamp argument from moment(...)
        let moment_end = find_matching_paren(expr, 7); // 7 is position of '(' in "moment("
        let timestamp_arg = &expr[7..moment_end];

        // Get the timestamp value
        let timestamp = if timestamp_arg == "Date.now()" || timestamp_arg.is_empty() {
            chrono::Utc::now().timestamp_millis()
        } else if let Ok(val) = self.evaluate_to_value(timestamp_arg) {
            match val {
                EjsValue::Number(n) => n as i64,
                EjsValue::String(s) => {
                    // Try to parse as date string
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
                        dt.timestamp_millis()
                    } else {
                        chrono::Utc::now().timestamp_millis()
                    }
                }
                _ => chrono::Utc::now().timestamp_millis(),
            }
        } else {
            chrono::Utc::now().timestamp_millis()
        };

        // Parse the rest of the chain to extract timezone, locale, and format
        let rest = &expr[moment_end + 1..];

        // Default values
        let mut timezone = "UTC".to_string();
        let mut locale = "en".to_string();
        let mut format = "YYYY-MM-DD HH:mm:ss".to_string();

        // Parse .tz('...')
        if let Some(tz_pos) = rest.find(".tz(") {
            let tz_start = tz_pos + 4;
            if let Some(tz_end) = rest[tz_start..].find(')') {
                let tz_arg = &rest[tz_start..tz_start + tz_end];
                timezone = tz_arg.trim_matches(|c| c == '\'' || c == '"').to_string();
            }
        }

        // Parse .locale('...')
        if let Some(loc_pos) = rest.find(".locale(") {
            let loc_start = loc_pos + 8;
            if let Some(loc_end) = rest[loc_start..].find(')') {
                let loc_arg = &rest[loc_start..loc_start + loc_end];
                locale = loc_arg.trim_matches(|c| c == '\'' || c == '"').to_string();
            }
        }

        // Parse .format('...')
        if let Some(fmt_pos) = rest.find(".format(") {
            let fmt_start = fmt_pos + 8;
            if let Some(fmt_end) = rest[fmt_start..].find(')') {
                let fmt_arg = &rest[fmt_start..fmt_start + fmt_end];
                format = fmt_arg.trim_matches(|c| c == '\'' || c == '"').to_string();
            }
        }

        // Convert timestamp to DateTime
        let dt =
            chrono::DateTime::from_timestamp_millis(timestamp).unwrap_or_else(chrono::Utc::now);

        // Apply timezone
        let formatted = match timezone.as_str() {
            "Asia/Shanghai" | "Asia/Hong_Kong" | "Asia/Taipei" => {
                let tz: chrono_tz::Tz = chrono_tz::Asia::Shanghai;
                let local_dt = dt.with_timezone(&tz);
                format_date_with_locale(&local_dt, &format, &locale)
            }
            "America/New_York" => {
                let tz: chrono_tz::Tz = chrono_tz::America::New_York;
                let local_dt = dt.with_timezone(&tz);
                format_date_with_locale(&local_dt, &format, &locale)
            }
            "Europe/London" => {
                let tz: chrono_tz::Tz = chrono_tz::Europe::London;
                let local_dt = dt.with_timezone(&tz);
                format_date_with_locale(&local_dt, &format, &locale)
            }
            _ => {
                // Use UTC as fallback
                format_date_with_locale(&dt, &format, &locale)
            }
        };

        Ok(EjsValue::String(formatted))
    }
}

// ============================================================================
// EjsTemplate and EjsEngine
// ============================================================================

/// Represents a parsed EJS template
#[derive(Debug, Clone)]
pub struct EjsTemplate {
    pub ast: Vec<AstNode>,
    #[allow(dead_code)]
    source: String,
}

/// For backwards compatibility
#[derive(Debug, Clone)]
pub enum EjsNode {
    Text(String),
    OutputEscaped(String),
    OutputRaw(String),
    Code(String),
    #[allow(dead_code)]
    Comment(String),
}

impl EjsTemplate {
    /// For backwards compatibility - return nodes in old format
    pub fn nodes(&self) -> Vec<EjsNode> {
        self.ast_to_nodes(&self.ast)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn ast_to_nodes(&self, ast: &[AstNode]) -> Vec<EjsNode> {
        let mut nodes = Vec::new();
        for node in ast {
            match node {
                AstNode::Text(s) => nodes.push(EjsNode::Text(s.clone())),
                AstNode::OutputEscaped(s) => nodes.push(EjsNode::OutputEscaped(s.clone())),
                AstNode::OutputRaw(s) => nodes.push(EjsNode::OutputRaw(s.clone())),
                AstNode::Comment(s) => nodes.push(EjsNode::Comment(s.clone())),
                AstNode::Code(s) => nodes.push(EjsNode::Code(s.clone())),
                AstNode::If {
                    condition,
                    then_branch,
                    ..
                } => {
                    nodes.push(EjsNode::Code(format!("if ({}) {{", condition)));
                    nodes.extend(self.ast_to_nodes(then_branch));
                    nodes.push(EjsNode::Code("}".to_string()));
                }
                AstNode::Each {
                    array_expr,
                    item_var,
                    index_var,
                    body,
                } => {
                    let idx = index_var.as_deref().unwrap_or("");
                    if idx.is_empty() {
                        nodes.push(EjsNode::Code(format!(
                            "{}.each(function({})",
                            array_expr, item_var
                        )));
                    } else {
                        nodes.push(EjsNode::Code(format!(
                            "{}.each(function({}, {})",
                            array_expr, item_var, idx
                        )));
                    }
                    nodes.extend(self.ast_to_nodes(body));
                    nodes.push(EjsNode::Code("})".to_string()));
                }
                AstNode::ForOf {
                    item_var,
                    iterable,
                    body,
                } => {
                    nodes.push(EjsNode::Code(format!(
                        "for (var {} of {}) {{",
                        item_var, iterable
                    )));
                    nodes.extend(self.ast_to_nodes(body));
                    nodes.push(EjsNode::Code("}".to_string()));
                }
                AstNode::ForIn {
                    key_var,
                    object_expr,
                    body,
                } => {
                    nodes.push(EjsNode::Code(format!(
                        "for (var {} in {}) {{",
                        key_var, object_expr
                    )));
                    nodes.extend(self.ast_to_nodes(body));
                    nodes.push(EjsNode::Code("}".to_string()));
                }
                AstNode::VarDecl { name, value } => {
                    nodes.push(EjsNode::Code(format!("var {} = {}", name, value)));
                }
                AstNode::Sequence(seq) => {
                    nodes.extend(self.ast_to_nodes(seq));
                }
            }
        }
        nodes
    }
}

/// The EJS template engine
#[derive(Clone)]
pub struct EjsEngine {
    templates: HashMap<String, EjsTemplate>,
    #[allow(dead_code)]
    open_delim: String,
    #[allow(dead_code)]
    close_delim: String,
}

impl EjsEngine {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            open_delim: "<%".to_string(),
            close_delim: "%>".to_string(),
        }
    }

    /// Parse an EJS template string
    pub fn parse(&self, source: &str) -> Result<EjsTemplate, EjsError> {
        // Lexer: source -> tokens
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;

        // Parser: tokens -> AST
        let mut parser = Parser::new(tokens);
        let ast = parser.parse()?;

        Ok(EjsTemplate {
            ast,
            source: source.to_string(),
        })
    }

    /// Register a template with a name
    pub fn register(&mut self, name: &str, source: &str) -> Result<(), EjsError> {
        let template = self.parse(source)?;
        self.templates.insert(name.to_string(), template);
        Ok(())
    }

    /// Get a registered template
    pub fn get(&self, name: &str) -> Option<&EjsTemplate> {
        self.templates.get(name)
    }

    /// Render a template with the given context
    pub fn render(&self, template: &EjsTemplate, context: &EjsContext) -> Result<String, EjsError> {
        let mut evaluator = Evaluator::new(context, None);
        evaluator.render(&template.ast)
    }

    /// Render a template with partials support
    pub fn render_with_partials(
        &self,
        template: &EjsTemplate,
        context: &EjsContext,
        partials: Arc<HashMap<String, String>>,
    ) -> Result<String, EjsError> {
        let mut evaluator = Evaluator::new(context, Some(partials));
        evaluator.render(&template.ast)
    }
}

impl Default for EjsEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// HTML escape a string
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Compare two values
fn compare_values(a: &EjsValue, b: &EjsValue) -> i32 {
    match (a, b) {
        (EjsValue::Number(a), EjsValue::Number(b)) => {
            if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            }
        }
        (EjsValue::String(a), EjsValue::String(b)) => a.cmp(b) as i32,
        _ => 0,
    }
}

/// Try to parse a string literal
fn try_parse_string_literal(expr: &str) -> Option<String> {
    let chars: Vec<char> = expr.chars().collect();
    if chars.is_empty() {
        return None;
    }

    let quote = chars[0];
    if quote != '"' && quote != '\'' {
        return None;
    }

    let mut i = 1;
    while i < chars.len() {
        if chars[i] == quote && (i == 0 || chars[i - 1] != '\\') {
            if i == chars.len() - 1 {
                return Some(expr[1..expr.len() - 1].to_string());
            } else {
                return None;
            }
        }
        i += 1;
    }

    None
}

/// Find the matching closing parenthesis for an opening paren at the given position
fn find_matching_paren(expr: &str, open_pos: usize) -> usize {
    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = ' ';
    let chars: Vec<char> = expr.chars().collect();

    for (i, &c) in chars.iter().enumerate().skip(open_pos) {
        if in_string {
            if c == string_char && (i == 0 || chars[i - 1] != '\\') {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' | '\'' => {
                in_string = true;
                string_char = c;
            }
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    expr.len() - 1 // fallback to end
}

/// Find an operator respecting parentheses and strings
fn find_operator(expr: &str, op: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = ' ';
    let chars: Vec<char> = expr.chars().collect();
    let op_chars: Vec<char> = op.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if in_string {
            if c == string_char && (i == 0 || chars[i - 1] != '\\') {
                in_string = false;
            }
            continue;
        }

        if c == '"' || c == '\'' {
            in_string = true;
            string_char = c;
            continue;
        }

        if c == '(' || c == '[' || c == '{' {
            depth += 1;
            continue;
        }

        if c == ')' || c == ']' || c == '}' {
            depth -= 1;
            continue;
        }

        if depth == 0 && i + op_chars.len() <= chars.len() {
            let matches = op_chars
                .iter()
                .enumerate()
                .all(|(j, oc)| chars[i + j] == *oc);
            if matches {
                return Some(i);
            }
        }
    }

    None
}

/// Find the + operator for string concatenation (not inside strings or parens)
fn find_plus_operator(expr: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = ' ';
    let chars: Vec<char> = expr.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if in_string {
            if c == string_char && (i == 0 || chars[i - 1] != '\\') {
                in_string = false;
            }
            continue;
        }

        if c == '"' || c == '\'' {
            in_string = true;
            string_char = c;
            continue;
        }

        if c == '(' || c == '[' || c == '{' {
            depth += 1;
            continue;
        }

        if c == ')' || c == ']' || c == '}' {
            depth -= 1;
            continue;
        }

        // Find + that's not part of ++ or +=
        if depth == 0 && c == '+' {
            let next = chars.get(i + 1);
            if next != Some(&'+') && next != Some(&'=') {
                return Some(i);
            }
        }
    }

    None
}

/// Split a ternary expression at the ?
fn split_ternary(expr: &str) -> Option<(&str, &str)> {
    let pos = find_operator(expr, "?")?;
    Some((&expr[..pos], &expr[pos + 1..]))
}

/// Parse function arguments
fn parse_function_args(args_str: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = ' ';

    for c in args_str.chars() {
        if in_string {
            current.push(c);
            if c == string_char {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' | '\'' => {
                in_string = true;
                string_char = c;
                current.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                args.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(c),
        }
    }

    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }

    args
}

/// Parse object literal pairs
fn parse_object_pairs(inner: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut in_value = false;
    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = ' ';

    for c in inner.chars() {
        if in_string {
            if in_value {
                current_value.push(c);
            } else {
                current_key.push(c);
            }
            if c == string_char {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' | '\'' => {
                in_string = true;
                string_char = c;
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
            '(' | '[' | '{' => {
                depth += 1;
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
            ')' | ']' | '}' => {
                depth -= 1;
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
            ':' if depth == 0 && !in_value => {
                in_value = true;
            }
            ',' if depth == 0 => {
                let key = current_key.trim().to_string();
                let value = current_value.trim().to_string();
                if !key.is_empty() {
                    pairs.push((key, value));
                }
                current_key = String::new();
                current_value = String::new();
                in_value = false;
            }
            _ => {
                if in_value {
                    current_value.push(c);
                } else {
                    current_key.push(c);
                }
            }
        }
    }

    let key = current_key.trim().to_string();
    let value = current_value.trim().to_string();
    if !key.is_empty() {
        pairs.push((key, value));
    }

    pairs
}

/// Extract condition from if statement
fn extract_condition(code: &str) -> String {
    let code = code.trim();

    let start = code.find('(');
    if let Some(start) = start {
        let mut depth = 0;
        let chars: Vec<char> = code[start..].chars().collect();
        for (i, c) in chars.iter().enumerate() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return code[start + 1..start + i].to_string();
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(rest) = code.strip_prefix("if ") {
        if let Some(brace) = rest.find('{') {
            return rest[..brace].trim().to_string();
        }
    }

    String::new()
}

/// Parse a for...of loop statement
fn parse_for_of_loop(code: &str) -> Option<(String, String)> {
    let code = code.trim();

    let start = code.find('(')?;
    let end = code.rfind(')')?;
    let inner = &code[start + 1..end];

    let parts: Vec<&str> = inner.splitn(2, " of ").collect();
    if parts.len() != 2 {
        return None;
    }

    let var_decl = parts[0].trim();
    let iterable = parts[1].trim();

    let var_name = var_decl
        .trim_start_matches("var ")
        .trim_start_matches("let ")
        .trim_start_matches("const ")
        .trim();

    Some((var_name.to_string(), iterable.to_string()))
}

/// Parse a for...in loop statement
fn parse_for_in_loop(code: &str) -> Option<(String, String)> {
    let code = code.trim();

    let start = code.find('(')?;
    let end = code.rfind(')')?;
    let inner = &code[start + 1..end];

    let parts: Vec<&str> = inner.splitn(2, " in ").collect();
    if parts.len() != 2 {
        return None;
    }

    let var_decl = parts[0].trim();
    let object_expr = parts[1].trim();

    let key_var = var_decl
        .trim_start_matches("var ")
        .trim_start_matches("let ")
        .trim_start_matches("const ")
        .trim();

    Some((key_var.to_string(), object_expr.to_string()))
}

/// Format a date using Moment.js-compatible format string
fn format_date_moment<Tz: chrono::TimeZone>(date: &chrono::DateTime<Tz>, format: &str) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let chrono_format = moment_to_chrono_format(format);
    date.format(&chrono_format).to_string()
}

/// Format a date with locale support
/// Handles Moment.js format strings with locale-specific replacements
fn format_date_with_locale<Tz: chrono::TimeZone>(
    date: &chrono::DateTime<Tz>,
    format: &str,
    locale: &str,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    // Handle 'a' (AM/PM) with locale
    let am_pm = if date.hour12().0 {
        match locale {
            "zh-cn" | "zh-tw" | "zh" => "下午",
            _ => "PM",
        }
    } else {
        match locale {
            "zh-cn" | "zh-tw" | "zh" => "上午",
            _ => "AM",
        }
    };

    // Replace 'a' with locale-specific AM/PM, and 'h' with 12-hour format
    let mut result = format.to_string();

    // Handle the 'a' token (must be done before other replacements)
    result = result.replace(" a ", &format!(" {} ", am_pm));
    result = result.replace(", a ", &format!(", {} ", am_pm));
    if result.ends_with(" a") {
        result = result[..result.len() - 2].to_string() + " " + am_pm;
    }

    // Handle 'h' for 12-hour format (without leading zero)
    let hour12 = date.hour12().1;
    result = result.replace(" h:", &format!(" {}:", hour12));

    // Now use the standard moment format conversion for the rest
    let chrono_format = moment_to_chrono_format(&result);

    // If the format still contains 'a', replace it
    let formatted = date.format(&chrono_format).to_string();
    formatted.replace(" a ", &format!(" {} ", am_pm))
}

/// Convert Moment.js format to chrono format
fn moment_to_chrono_format(format: &str) -> String {
    let replacements = [
        ("YYYY", "%Y"),
        ("YY", "%y"),
        ("MMMM", "%B"),
        ("MMM", "%b"),
        ("MM", "%m"),
        ("DDDD", "%j"),
        ("DD", "%d"),
        ("HH", "%H"),
        ("hh", "%I"),
        ("mm", "%M"),
        ("ss", "%S"),
        ("dddd", "%A"),
        ("ddd", "%a"),
        ("ZZ", "%z"),
        ("SSS", "%3f"),
    ];

    let mut result = format.to_string();
    for (from, to) in replacements {
        result = result.replace(from, to);
    }
    result
}

/// Strip HTML tags from a string
fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;

    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    result
}

/// Parse a date string and format it
fn parse_and_format_date(date_str: &str, format: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
        return format_date_moment(&dt, format);
    }

    let formats = [
        "%Y-%m-%dT%H:%M:%S%.3f%:z",
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d",
    ];

    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, fmt) {
            let local_dt = chrono::Local.from_local_datetime(&dt).unwrap();
            return format_date_moment(&local_dt, format);
        }
    }

    date_str.to_string()
}

/// Parse a date string and format it in ISO 8601/XML format
fn parse_and_format_date_xml(date_str: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_str) {
        return dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
    }

    let formats = [
        "%Y-%m-%dT%H:%M:%S%.3f%:z",
        "%Y-%m-%dT%H:%M:%S%:z",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d",
    ];

    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(date_str, fmt) {
            let local_dt = chrono::Local.from_local_datetime(&dt).unwrap();
            return local_dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        }
    }

    date_str.to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_simple() {
        let mut lexer = Lexer::new("Hello <%= name %>!");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[0], Token::Text(t) if t == "Hello "));
        assert!(matches!(&tokens[1], Token::OutputEscaped(e) if e == "name"));
        assert!(matches!(&tokens[2], Token::Text(t) if t == "!"));
    }

    #[test]
    fn test_lexer_code_block() {
        let mut lexer = Lexer::new("<% if (x) { %>yes<% } %>");
        let tokens = lexer.tokenize().unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(&tokens[0], Token::Code(c) if c == "if (x) {"));
        assert!(matches!(&tokens[1], Token::Text(t) if t == "yes"));
        assert!(matches!(&tokens[2], Token::Code(c) if c == "}"));
    }

    #[test]
    fn test_parse_simple_template() {
        let engine = EjsEngine::new();
        let template = engine.parse("Hello <%= name %>!").unwrap();
        assert_eq!(template.ast.len(), 3);
    }

    #[test]
    fn test_render_simple_template() {
        let engine = EjsEngine::new();
        let template = engine.parse("Hello <%= name %>!").unwrap();

        let mut context = EjsContext::new();
        context.set_string("name", "World");

        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "Hello World!");
    }

    #[test]
    fn test_render_escaped_html() {
        let engine = EjsEngine::new();
        let template = engine.parse("<%= content %>").unwrap();

        let mut context = EjsContext::new();
        context.set_string("content", "<script>alert('xss')</script>");

        let result = engine.render(&template, &context).unwrap();
        assert!(result.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_render_raw_html() {
        let engine = EjsEngine::new();
        let template = engine.parse("<%- content %>").unwrap();

        let mut context = EjsContext::new();
        context.set_string("content", "<p>Hello</p>");

        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "<p>Hello</p>");
    }

    #[test]
    fn test_property_access() {
        let engine = EjsEngine::new();
        let template = engine.parse("<%= page.title %>").unwrap();

        let mut context = EjsContext::new();
        let mut page = IndexMap::new();
        page.insert(
            "title".to_string(),
            EjsValue::String("My Title".to_string()),
        );
        context.set("page", EjsValue::Object(page));

        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "My Title");
    }

    #[test]
    fn test_comparison_operators() {
        let engine = EjsEngine::new();

        let template = engine.parse("<%= 'a' !== 'b' %>").unwrap();
        let context = EjsContext::new();
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "true");

        let template = engine.parse("<%= 'a' === 'a' %>").unwrap();
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "true");
    }

    #[test]
    fn test_if_statement() {
        let engine = EjsEngine::new();
        let template = engine.parse("<% if (show) { %>visible<% } %>").unwrap();

        let mut context = EjsContext::new();
        context.set_bool("show", true);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "visible");

        context.set_bool("show", false);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_if_else_statement() {
        let engine = EjsEngine::new();
        let template = engine
            .parse("<% if (show) { %>yes<% } else { %>no<% } %>")
            .unwrap();

        let mut context = EjsContext::new();
        context.set_bool("show", true);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "yes");

        context.set_bool("show", false);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "no");
    }

    #[test]
    fn test_each_loop() {
        let engine = EjsEngine::new();
        let template = engine
            .parse("<% items.each(function(item){ %><%= item %>,<% }) %>")
            .unwrap();

        let mut context = EjsContext::new();
        context.set(
            "items",
            EjsValue::Array(vec![
                EjsValue::String("a".to_string()),
                EjsValue::String("b".to_string()),
                EjsValue::String("c".to_string()),
            ]),
        );

        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "a,b,c,");
    }

    #[test]
    fn test_each_loop_with_index() {
        let engine = EjsEngine::new();
        let template = engine
            .parse("<% items.forEach(function(item, i){ %><%= i %>:<%= item %> <% }) %>")
            .unwrap();

        let mut context = EjsContext::new();
        context.set(
            "items",
            EjsValue::Array(vec![
                EjsValue::String("x".to_string()),
                EjsValue::String("y".to_string()),
            ]),
        );

        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "0:x 1:y ");
    }

    #[test]
    fn test_set_nested_property() {
        let engine = EjsEngine::new();

        let mut context = EjsContext::new();
        let mut page = IndexMap::new();
        page.insert("title".to_string(), EjsValue::String("Index".to_string()));
        context.set("page", EjsValue::Object(page));

        context.set_nested(
            "page.posts",
            EjsValue::Array(vec![
                EjsValue::String("Post 1".to_string()),
                EjsValue::String("Post 2".to_string()),
            ]),
        );

        let template = engine.parse("<%= page.title %>").unwrap();
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "Index");

        let template = engine
            .parse("<% page.posts.each(function(post){ %><%= post %>,<% }) %>")
            .unwrap();
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "Post 1,Post 2,");
    }

    #[test]
    fn test_css_helper() {
        let engine = EjsEngine::new();
        let template = engine.parse("<%- css('css/style') %>").unwrap();

        let mut context = EjsContext::new();
        let mut config = IndexMap::new();
        config.insert("root".to_string(), EjsValue::String("/".to_string()));
        context.set("config", EjsValue::Object(config));

        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, r#"<link rel="stylesheet" href="/css/style.css">"#);
    }

    #[test]
    fn test_false_if_with_script() {
        let engine = EjsEngine::new();
        let template = engine
            .parse("<% if (show) { %><script>var x = 'test';</script><% } %>AFTER")
            .unwrap();

        let mut context = EjsContext::new();
        context.set_bool("show", false);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "AFTER");
    }

    #[test]
    fn test_false_if_with_nested_if() {
        let engine = EjsEngine::new();
        let template_src = r#"<% if (outer) { %>
<script>
var x = '<% if (inner) { %>yes<% } else { %>no<% } %>';
</script>
<% } %>
AFTER"#;
        let template = engine.parse(template_src).unwrap();

        let mut context = EjsContext::new();
        context.set_bool("outer", false);
        context.set_bool("inner", true);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result.trim(), "AFTER");
    }

    #[test]
    fn test_nested_if_with_outer_true() {
        let engine = EjsEngine::new();
        let template_src = r#"<% if (outer) { %>OUTER<% if (inner) { %>INNER<% } %><% } %>END"#;
        let template = engine.parse(template_src).unwrap();

        let mut context = EjsContext::new();
        context.set_bool("outer", true);
        context.set_bool("inner", true);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "OUTERINNEREND");

        context.set_bool("inner", false);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "OUTEREND");
    }

    #[test]
    fn test_if_else_if_else() {
        let engine = EjsEngine::new();
        let template = engine
            .parse(
                "<% if (x === 1) { %>one<% } else if (x === 2) { %>two<% } else { %>other<% } %>",
            )
            .unwrap();

        let mut context = EjsContext::new();
        context.set_number("x", 1.0);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "one");

        context.set_number("x", 2.0);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "two");

        context.set_number("x", 3.0);
        let result = engine.render(&template, &context).unwrap();
        assert_eq!(result, "other");
    }
}

#[test]
fn test_for_in_loop() {
    let engine = EjsEngine::new();
    let template = engine
        .parse("<% for (var key in obj) { %><%= key %>:<%= obj[key] %> <% } %>")
        .unwrap();

    let mut context = EjsContext::new();
    let mut obj = IndexMap::new();
    obj.insert("a".to_string(), EjsValue::String("1".to_string()));
    obj.insert("b".to_string(), EjsValue::String("2".to_string()));
    context.set("obj", EjsValue::Object(obj));

    let result = engine.render(&template, &context).unwrap();
    // Object keys may be in any order
    assert!(result.contains("a:1"));
    assert!(result.contains("b:2"));
}

#[test]
fn test_for_in_with_dynamic_access() {
    let engine = EjsEngine::new();
    let template = engine
        .parse("<% for (var i in menu) { %><a href=\"<%= menu[i] %>\"><%= i %></a><% } %>")
        .unwrap();

    let mut context = EjsContext::new();
    let mut menu = IndexMap::new();
    menu.insert("Home".to_string(), EjsValue::String("/".to_string()));
    menu.insert(
        "Archives".to_string(),
        EjsValue::String("/archives".to_string()),
    );
    context.set("menu", EjsValue::Object(menu));

    let result = engine.render(&template, &context).unwrap();
    assert!(result.contains(r#"<a href="/">"#));
    assert!(result.contains(">Home</a>"));
    assert!(result.contains(r#"<a href="/archives">"#));
    assert!(result.contains(">Archives</a>"));
}
