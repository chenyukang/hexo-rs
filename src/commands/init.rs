//! Initialize a new Hexo site

use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::Hexo;

/// Initialize a new site in the given directory
pub fn init_site(target_dir: &Path) -> Result<()> {
    // Create directory structure
    fs::create_dir_all(target_dir)?;
    fs::create_dir_all(target_dir.join("source/_posts"))?;
    fs::create_dir_all(target_dir.join("source/_drafts"))?;
    fs::create_dir_all(target_dir.join("themes"))?;
    fs::create_dir_all(target_dir.join("scaffolds"))?;

    // Create default _config.yml
    let config_content = r#"# Hexo Configuration
## Docs: https://hexo.io/docs/configuration.html

# Site
title: Hexo
subtitle: ''
description: ''
keywords:
author: John Doe
language: en
timezone: ''

# URL
url: http://example.com
root: /
permalink: :year/:month/:day/:title/
permalink_defaults:
pretty_urls:
  trailing_index: true
  trailing_html: true

# Directory
source_dir: source
public_dir: public
tag_dir: tags
archive_dir: archives
category_dir: categories
code_dir: downloads/code
i18n_dir: :lang
skip_render:

# Writing
new_post_name: :title.md
default_layout: post
titlecase: false
external_link:
  enable: true
  field: site
  exclude: []
filename_case: 0
render_drafts: false
post_asset_folder: false
relative_link: false
future: true
syntax_highlighter: highlight.js
highlight:
  line_number: true
  auto_detect: false
  tab_replace: ''
  wrap: true
  hljs: false
prismjs:
  preprocess: true
  line_number: true
  tab_replace: ''

# Home page setting
index_generator:
  path: ''
  per_page: 10
  order_by: -date

# Category & Tag
default_category: uncategorized
category_map:
tag_map:

# Metadata elements
meta_generator: true

# Date / Time format
date_format: YYYY-MM-DD
time_format: HH:mm:ss
updated_option: mtime

# Pagination
per_page: 10
pagination_dir: page

# Extensions
theme: landscape
"#;

    fs::write(target_dir.join("_config.yml"), config_content)?;

    // Create scaffold templates
    let post_scaffold = r#"---
title: {{ title }}
date: {{ date }}
tags:
---
"#;

    let page_scaffold = r#"---
title: {{ title }}
date: {{ date }}
---
"#;

    let draft_scaffold = r#"---
title: {{ title }}
tags:
---
"#;

    fs::write(target_dir.join("scaffolds/post.md"), post_scaffold)?;
    fs::write(target_dir.join("scaffolds/page.md"), page_scaffold)?;
    fs::write(target_dir.join("scaffolds/draft.md"), draft_scaffold)?;

    // Create a sample post
    let now = chrono::Local::now();
    let sample_post = format!(
        r#"---
title: Hello World
date: {}
tags:
---

Welcome to [Hexo](https://hexo.io/)! This is your very first post. Check [documentation](https://hexo.io/docs/) for more info. If you get any problems when using Hexo, you can find the answer in [troubleshooting](https://hexo.io/docs/troubleshooting.html) or you can ask me on [GitHub](https://github.com/hexojs/hexo/issues).

## Quick Start

### Create a new post

```bash
$ hexo new "My New Post"
```

More info: [Writing](https://hexo.io/docs/writing.html)

### Run server

```bash
$ hexo server
```

More info: [Server](https://hexo.io/docs/server.html)

### Generate static files

```bash
$ hexo generate
```

More info: [Generating](https://hexo.io/docs/generating.html)

### Deploy to remote sites

```bash
$ hexo deploy
```

More info: [Deployment](https://hexo.io/docs/one-command-deployment.html)
"#,
        now.format("%Y-%m-%d %H:%M:%S")
    );

    fs::write(target_dir.join("source/_posts/hello-world.md"), sample_post)?;

    Ok(())
}

/// Run the init command with an existing Hexo instance
pub fn run(hexo: &Hexo) -> Result<()> {
    init_site(&hexo.base_dir)
}
