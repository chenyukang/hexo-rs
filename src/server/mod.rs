//! Development server with live reload

use anyhow::Result;
use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::{Request, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

use crate::Hexo;

/// Live reload script injected into HTML pages
const LIVE_RELOAD_SCRIPT: &str = r#"
<script>
(function() {
    var ws = new WebSocket('ws://' + location.host + '/__livereload');
    ws.onmessage = function(msg) {
        if (msg.data === 'reload') {
            location.reload();
        }
    };
    ws.onclose = function() {
        console.log('Live reload disconnected. Attempting to reconnect...');
        setTimeout(function() { location.reload(); }, 1000);
    };
})();
</script>
</body>
"#;

/// Server state
struct ServerState {
    public_dir: PathBuf,
    reload_tx: broadcast::Sender<()>,
    live_reload: bool,
}

/// Start the development server
pub async fn start(hexo: &Hexo, ip: &str, port: u16, watch: bool, open: bool) -> Result<()> {
    // Create broadcast channel for live reload notifications
    let (reload_tx, _) = broadcast::channel::<()>(16);

    let state = Arc::new(ServerState {
        public_dir: hexo.public_dir.clone(),
        reload_tx: reload_tx.clone(),
        live_reload: watch,
    });

    // Create router with live reload endpoint
    let app = Router::new()
        .route("/__livereload", get(livereload_handler))
        .fallback(fallback_handler)
        .with_state(state);

    // Parse address - handle "localhost" specially
    let bind_ip = if ip == "localhost" { "127.0.0.1" } else { ip };
    let addr: SocketAddr = format!("{}:{}", bind_ip, port).parse()?;

    let url = format!("http://{}:{}", ip, port);
    println!("Server running at {}", url);
    if watch {
        println!("Live reload enabled. Watching for changes...");
    }
    println!("Press Ctrl+C to stop.");

    // Open browser if requested
    if open {
        if let Err(e) = open_browser(&url) {
            tracing::warn!("Failed to open browser: {}", e);
        }
    }

    // Start file watcher if watch mode is enabled
    if watch {
        let source_dir = hexo.source_dir.clone();
        let theme_dir = hexo.theme_dir.clone();
        let config_path = hexo.base_dir.join("_config.yml");
        let hexo_clone = hexo.clone();

        tokio::spawn(async move {
            if let Err(e) =
                watch_and_reload(source_dir, theme_dir, config_path, hexo_clone, reload_tx).await
            {
                tracing::error!("File watcher error: {}", e);
            }
        });
    }

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Watch for file changes and trigger reload
async fn watch_and_reload(
    source_dir: PathBuf,
    theme_dir: PathBuf,
    config_path: PathBuf,
    hexo: Hexo,
    reload_tx: broadcast::Sender<()>,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    // Create debouncer to avoid multiple rapid rebuilds
    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;

    // Watch source directory
    if source_dir.exists() {
        debouncer
            .watcher()
            .watch(&source_dir, RecursiveMode::Recursive)?;
        tracing::debug!("Watching: {:?}", source_dir);
    }

    // Watch theme directory
    if theme_dir.exists() {
        debouncer
            .watcher()
            .watch(&theme_dir, RecursiveMode::Recursive)?;
        tracing::debug!("Watching: {:?}", theme_dir);
    }

    // Watch config file
    if config_path.exists() {
        debouncer
            .watcher()
            .watch(&config_path, RecursiveMode::NonRecursive)?;
        tracing::debug!("Watching: {:?}", config_path);
    }

    // Handle file change events
    loop {
        match rx.recv() {
            Ok(Ok(events)) => {
                // Filter out irrelevant events (like .git, .DS_Store, etc.)
                let relevant_events: Vec<_> = events
                    .iter()
                    .filter(|e| {
                        let path_str = e.path.to_string_lossy();
                        !path_str.contains(".git")
                            && !path_str.contains(".DS_Store")
                            && !path_str.contains("node_modules")
                            && !path_str.ends_with('~')
                    })
                    .collect();

                if relevant_events.is_empty() {
                    continue;
                }

                // Log changed files
                println!();
                for event in &relevant_events {
                    println!("📝 File changed: {}", event.path.display());
                }

                // Regenerate site
                println!("\n🔄 Regenerating...");
                match hexo.generate() {
                    Ok(_) => {
                        println!("✅ Regenerated successfully!");
                        // Notify all connected clients to reload
                        let _ = reload_tx.send(());
                    }
                    Err(e) => {
                        println!("❌ Generation failed: {}", e);
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!("Watch error: {:?}", e);
            }
            Err(e) => {
                tracing::error!("Channel error: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

/// WebSocket handler for live reload
async fn livereload_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    let reload_rx = state.reload_tx.subscribe();
    ws.on_upgrade(move |socket| handle_livereload_socket(socket, reload_rx))
}

/// Handle WebSocket connection for live reload
async fn handle_livereload_socket(mut socket: WebSocket, mut reload_rx: broadcast::Receiver<()>) {
    tracing::debug!("Live reload client connected");

    loop {
        tokio::select! {
            // Wait for reload signal
            result = reload_rx.recv() => {
                match result {
                    Ok(_) => {
                        if socket.send(Message::Text("reload".to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            // Handle incoming messages (ping/pong)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Ping(data))) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    tracing::debug!("Live reload client disconnected");
}

/// Fallback handler that serves files and injects live reload script
async fn fallback_handler(
    State(state): State<Arc<ServerState>>,
    request: Request<Body>,
) -> Response {
    let path = request.uri().path();

    // Determine the file path
    let file_path = if path == "/" {
        state.public_dir.join("index.html")
    } else {
        let clean_path = path.trim_start_matches('/');
        let candidate = state.public_dir.join(clean_path);

        // If it's a directory, look for index.html
        if candidate.is_dir() {
            candidate.join("index.html")
        } else if candidate.exists() {
            candidate
        } else {
            // Try adding .html extension
            let with_html = state.public_dir.join(format!("{}.html", clean_path));
            if with_html.exists() {
                with_html
            } else {
                candidate
            }
        }
    };

    // Check if it's an HTML file that needs live reload injection
    let is_html = file_path
        .extension()
        .map(|ext| ext == "html" || ext == "htm")
        .unwrap_or(false)
        || file_path.ends_with("index.html");

    if is_html && state.live_reload {
        // Read and inject live reload script
        match tokio::fs::read_to_string(&file_path).await {
            Ok(content) => {
                let injected = inject_live_reload(&content);
                Html(injected).into_response()
            }
            Err(_) => (StatusCode::NOT_FOUND, "Not found").into_response(),
        }
    } else {
        // Serve static file using tower-http
        let mut service = ServeDir::new(&state.public_dir).append_index_html_on_directories(true);
        match service.try_call(request).await {
            Ok(response) => response.into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Server error").into_response(),
        }
    }
}

/// Inject live reload script into HTML content
fn inject_live_reload(html: &str) -> String {
    if html.contains("</body>") {
        html.replace("</body>", LIVE_RELOAD_SCRIPT)
    } else {
        // If no </body> tag, append to end
        format!("{}{}", html, LIVE_RELOAD_SCRIPT)
    }
}

/// Open a URL in the default browser
fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()?;
    }

    Ok(())
}
