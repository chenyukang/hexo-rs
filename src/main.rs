//! CLI entry point for hexo-rs

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "hexo-rs")]
#[command(author = "Yukang Chen")]
#[command(version = "0.1.0")]
#[command(about = "A fast static site generator compatible with Hexo themes", long_about = None)]
struct Cli {
    /// Set the base directory (defaults to current directory)
    #[arg(short, long, global = true)]
    cwd: Option<PathBuf>,

    /// Enable debug output
    #[arg(short, long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Hexo site
    Init {
        /// Directory to initialize (defaults to current directory)
        #[arg(default_value = ".")]
        folder: PathBuf,
    },

    /// Create a new post or page
    New {
        /// Layout to use (post, page, draft)
        #[arg(short, long, default_value = "post")]
        layout: String,

        /// Title of the new post
        title: String,

        /// Path for the new post
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Generate static files
    #[command(alias = "g")]
    Generate {
        /// Watch for file changes
        #[arg(short, long)]
        watch: bool,

        /// Deploy after generation
        #[arg(long)]
        deploy: bool,
    },

    /// Start a local server
    #[command(alias = "s")]
    Server {
        /// Port to listen on
        #[arg(short, long, default_value = "4000")]
        port: u16,

        /// IP address to bind to
        #[arg(short, long, default_value = "localhost")]
        ip: String,

        /// Open browser automatically
        #[arg(short, long)]
        open: bool,

        /// Enable static mode (no file watching)
        #[arg(long)]
        r#static: bool,
    },

    /// Clean the public folder and cache
    Clean,

    /// List site information
    List {
        /// Type of content to list (post, page, route, tag, category)
        #[arg(default_value = "post")]
        r#type: String,
    },

    /// Display version information
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.debug {
        "hexo_rs=debug,info"
    } else {
        "hexo_rs=info"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Determine base directory
    let base_dir = cli.cwd.unwrap_or_else(|| std::env::current_dir().unwrap());

    match cli.command {
        Commands::Init { folder } => {
            let target_dir = if folder.is_absolute() {
                folder
            } else {
                base_dir.join(folder)
            };
            tracing::info!("Initializing Hexo site in {:?}", target_dir);
            hexo_rs::commands::init::init_site(&target_dir)?;
            println!("Initialized empty Hexo site in {:?}", target_dir);
        }

        Commands::New {
            layout,
            title,
            path,
        } => {
            let hexo = hexo_rs::Hexo::new(&base_dir)?;
            tracing::info!("Creating new {} with title: {}", layout, title);
            hexo_rs::commands::new::create_post(&hexo, &title, &layout, path.as_deref())?;
        }

        Commands::Generate { watch, deploy: _ } => {
            let hexo = hexo_rs::Hexo::new(&base_dir)?;
            tracing::info!("Generating static files...");

            hexo_rs::commands::generate::run(&hexo)?;
            println!("Generated successfully!");

            if watch {
                tracing::info!("Watching for file changes...");
                hexo_rs::commands::generate::watch(&hexo).await?;
            }
        }

        Commands::Server {
            port,
            ip,
            open,
            r#static,
        } => {
            let hexo = hexo_rs::Hexo::new(&base_dir)?;

            // Generate first
            tracing::info!("Generating static files...");
            hexo.generate()?;

            tracing::info!("Starting server at http://{}:{}", ip, port);
            hexo_rs::server::start(&hexo, &ip, port, !r#static, open).await?;
        }

        Commands::Clean => {
            let hexo = hexo_rs::Hexo::new(&base_dir)?;
            tracing::info!("Cleaning public folder...");
            hexo.clean()?;
            println!("Cleaned successfully!");
        }

        Commands::List { r#type } => {
            let hexo = hexo_rs::Hexo::new(&base_dir)?;
            hexo_rs::commands::list::run(&hexo, &r#type)?;
        }

        Commands::Version => {
            println!("hexo-rs version {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
