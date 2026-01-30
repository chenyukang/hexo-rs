# hexo-rs

A static site generator written in Rust, designed to be compatible with Hexo themes (EJS templates).

My blog is using Hexo, but I don't want to install Node.js and all the dependencies just to generate static files. So I wrote this project to generate Hexo sites using Rust.

**This project was built entirely through vibe coding with AI!**

Note: hexo-rs don't support all Hexo features, please read the "Limitations" section below.

## Features

- Fast: generates sites faster than the Node.js version of Hexo
- Compatible: Supports most Hexo EJS themes
- Simple: Command-line interface is basically consistent with Hexo

## Installation

```bash
cargo install hexo-rs
```

Or with cargo-binstall (faster, downloads prebuilt binary):

```bash
cargo binstall hexo-rs
```

## Usage

```bash
# Generate static files
hexo-rs generate

# Start local server
hexo-rs server

# Clean generated files
hexo-rs clean

# Create new post
hexo-rs new "Post Title"

# List posts
hexo-rs list
```

## Limitations

### 1. Stylus CSS Preprocessor

hexo-rs does not include a built-in Stylus compiler. If the theme uses `.styl` files, you need to:

**Option 1: Install stylus**

```bash
npm install -g stylus
```

hexo-rs will try to call `npx stylus` to compile, but this requires a Node.js environment.

**Option 2: Pre-compile CSS (Recommended)**

```bash
# Generate once using Node.js version of Hexo
npx hexo generate

# Copy the compiled CSS to the theme directory
cp public/css/style.css themes/your-theme/source/css/style.css
```

### 2. EJS Template Support

Most EJS syntax is supported, but the following features may not be fully compatible:

- Complex JavaScript expressions (executed using QuickJS engine)
- Some helper functions provided by Hexo plugins
- Complex parameter passing in `<%- partial(...) %>`

### 3. Unsupported Hexo Features

- Hexo plugin system
- Custom Generators
- Custom Helpers (only built-in helpers are supported)
- Deploy functionality (`hexo deploy`)
- Limited draft support

### 4. Markdown Rendering

Uses `pulldown-cmark` to render Markdown, which may have subtle differences from Hexo's default `marked` or `markdown-it`:

- Code highlighting uses `syntect`
- Some Hexo tag plugin syntax is not supported

### 5. Theme Configuration

The order of configuration items in theme configuration files (`_config.yml`) is preserved, but some complex YAML structures may be parsed differently.

## Tested Themes

- vexo

## Notes

1. **Before first use**: It's recommended to generate once with the Node.js version of Hexo to ensure the theme's CSS is compiled
2. **Post Front Matter**: Ensure the format is correct, recommended date format is `YYYY-MM-DD HH:mm:ss`
3. **File watching**: `hexo-rs server` will automatically watch for file changes and regenerate
4. **Debug mode**: Use `hexo-rs -d generate` to view detailed logs

## Development

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Code linting
cargo clippy
```

## License

MIT
