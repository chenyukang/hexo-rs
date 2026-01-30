# hexo-rs

一个用 Rust 编写的静态站点生成器，目标是兼容 Hexo 主题（EJS 模板）。

## 特性

- 快速：使用 Rust 编写，生成速度比 Node.js 版本的 Hexo 快数倍
- 兼容：支持大部分 Hexo EJS 主题
- 简单：命令行接口与 Hexo 基本一致

## 安装

```bash
cargo install --path .
```

## 使用

```bash
# 生成静态文件
hexo-rs generate

# 启动本地服务器
hexo-rs server

# 清理生成的文件
hexo-rs clean

# 创建新文章
hexo-rs new "文章标题"

# 列出文章
hexo-rs list
```

## 局限性

### 1. Stylus CSS 预处理器

hexo-rs 不内置 Stylus 编译器。如果主题使用 `.styl` 文件，需要：

**方案一：安装 stylus**

```bash
npm install -g stylus
```

hexo-rs 会尝试调用 `npx stylus` 来编译，但这需要 Node.js 环境。

**方案二：预编译 CSS（推荐）**

```bash
# 使用 Node.js 版 Hexo 生成一次
npx hexo generate

# 将编译好的 CSS 复制到主题目录
cp public/css/style.css themes/your-theme/source/css/style.css
```

### 2. EJS 模板支持

支持大部分 EJS 语法，但以下特性可能不完全兼容：

- 复杂的 JavaScript 表达式（使用 QuickJS 引擎执行）
- 某些 Hexo 插件提供的 helper 函数
- `<%- partial(...) %>` 中的复杂参数传递

### 3. 不支持的 Hexo 功能

- Hexo 插件系统
- 自定义 Generator
- 自定义 Helper（仅支持内置 helper）
- 部署功能（`hexo deploy`）
- 草稿功能有限支持

### 4. Markdown 渲染

使用 `pulldown-cmark` 渲染 Markdown，与 Hexo 默认的 `marked` 或 `markdown-it` 可能有细微差异：

- 代码高亮使用 `syntect`
- 某些 Hexo 标签插件语法不支持

### 5. 主题配置

主题配置文件（`_config.yml`）中的配置项顺序会被保留，但某些复杂的 YAML 结构可能解析不同。

## 已测试的主题

- vexo

## 注意事项

1. **首次使用前**：建议先用 Node.js 版 Hexo 生成一次，确保主题的 CSS 已编译
2. **文章 Front Matter**：确保格式正确，日期格式推荐使用 `YYYY-MM-DD HH:mm:ss`
3. **文件监听**：`hexo-rs server` 会自动监听文件变化并重新生成
4. **调试模式**：使用 `hexo-rs -d generate` 查看详细日志

## 开发

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# 运行测试
cargo test

# 代码检查
cargo clippy
```

## License

MIT
