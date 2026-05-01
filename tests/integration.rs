use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use vaserve::config::{self, AppConfig, CleanUrlsConfig, DirectoryListingConfig, RewriteRule};
use vaserve::handler::{self, HandlerState, SharedState};
use vaserve::templates;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::sleep;

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Test fixtures directory setup with unique directory per test
fn setup_test_dir() -> PathBuf {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("testdata_{}", id));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("subdir")).unwrap();
    std::fs::write(dir.join("index.html"), "<html><body>Hello</body></html>").unwrap();
    std::fs::write(dir.join("app.js"), "console.log('test');").unwrap();
    std::fs::write(dir.join("data.txt"), "plain text").unwrap();
    std::fs::write(dir.join("subdir/index.html"), "<html><body>Sub</body></html>").unwrap();
    dir
}


/// Helper to create a test server and return the port
async fn start_test_server(config: AppConfig) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let state: SharedState = Arc::new(HandlerState::new(config));

    let app = Router::new()
        .fallback(handler::handle_request)
        .with_state(state);

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // Wait for the server to be ready
    for _ in 0..50 {
        if reqwest::get(format!("http://127.0.0.1:{}", port)).await.is_ok() {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    port
}

fn default_config(dir: &PathBuf) -> AppConfig {
    AppConfig {
        public: dir.to_string_lossy().to_string(),
        endpoints: vec![],
        single: false,
        debug: false,
        no_request_logging: true,
        cors: false,
        no_clipboard: true,
        no_compression: false,
        no_etag: false,
        symlinks: false,
        ssl_cert: None,
        ssl_key: None,
        ssl_pass: None,
        no_port_switching: false,
        clean_urls: CleanUrlsConfig::Bool(false),
        rewrites: vec![],
        redirects: vec![],
        custom_headers: vec![],
        directory_listing: DirectoryListingConfig::Bool(true),
        unlisted: vec![],
        trailing_slash: None,
    }
}

fn get_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_gzip()
        .build()
        .unwrap()
}

// ============================================================
// Basic File Serving Tests
// ============================================================

#[tokio::test]
async fn test_serve_index_html() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/index.html", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
    let body = resp.text().await.unwrap();
    assert!(body.contains("Hello"));
}

#[tokio::test]
async fn test_serve_javascript() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/app.js", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("javascript") || content_type.contains("ecmascript"));
    let body = resp.text().await.unwrap();
    assert_eq!(body.trim(), "console.log('test');");
}

#[tokio::test]
async fn test_serve_plain_text() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/data.txt", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/plain"));
    let body = resp.text().await.unwrap();
    assert_eq!(body.trim(), "plain text");
}

#[tokio::test]
async fn test_404_not_found() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/nonexistent.html", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_etag_header_present() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/index.html", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().contains_key("etag"));
}

#[tokio::test]
async fn test_content_length_header() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/data.txt", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_length: u64 = resp.headers().get("content-length").unwrap().to_str().unwrap().parse().unwrap();
    assert!(content_length > 0);
}

#[tokio::test]
async fn test_content_disposition_inline() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/index.html", port))
        .send()
        .await
        .unwrap();

    let cd = resp.headers().get("content-disposition").unwrap().to_str().unwrap();
    assert!(cd.contains("inline"));
}

#[tokio::test]
async fn test_accept_ranges_header() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/index.html", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("accept-ranges").unwrap().to_str().unwrap(), "bytes");
}

// ============================================================
// Range Request Tests
// ============================================================

#[tokio::test]
async fn test_range_request() {
    let dir = setup_test_dir();
    // Write a known-size file
    std::fs::write(dir.join("range.txt"), "abcdefghijklmnopqrstuvwxyz").unwrap();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/range.txt", port))
        .header("Range", "bytes=0-4")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 206);
    let content_range = resp.headers().get("content-range").unwrap().to_str().unwrap().to_string();
    assert!(content_range.contains("bytes 0-4/26"));
    let body = resp.text().await.unwrap();
    assert_eq!(body, "abcde");
}

#[tokio::test]
async fn test_range_request_suffix() {
    let dir = setup_test_dir();
    std::fs::write(dir.join("range.txt"), "abcdefghijklmnopqrstuvwxyz").unwrap();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/range.txt", port))
        .header("Range", "bytes=-5")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 206);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "vwxyz");
}

// ============================================================
// Directory Listing Tests
// ============================================================

#[tokio::test]
async fn test_directory_listing_html() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
    let body = resp.text().await.unwrap();
    assert!(body.contains("Index of"));
    assert!(body.contains("index.html"));
    assert!(body.contains("app.js"));
    assert!(body.contains("data.txt"));
    assert!(body.contains("subdir"));
    assert!(body.contains("folder")); // Folder icon class
    assert!(body.contains("file"));   // File icon class
}

#[tokio::test]
async fn test_directory_listing_json() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/", port))
        .header("Accept", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn test_directory_listing_sorted_dirs_first() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/", port))
        .header("Accept", "application/json")
        .send()
        .await
        .unwrap();

    let json: serde_json::Value = resp.json().await.unwrap();
    let files = json["files"].as_array().unwrap();

    // Find position of subdir and data.txt
    let subdir_pos = files.iter().position(|f| f["base"] == "subdir").unwrap();
    let data_pos = files.iter().position(|f| f["base"] == "data.txt").unwrap();

    // Directories should come before files
    assert!(subdir_pos < data_pos, "Directories should be listed before files");
}

// ============================================================
// SPA Mode Tests
// ============================================================

#[tokio::test]
async fn test_spa_mode_rewrites_to_index() {
    let dir = setup_test_dir();
    let mut config = default_config(&dir);
    config.single = true;
    config.rewrites.push(RewriteRule {
        source: "**".to_string(),
        destination: "/index.html".to_string(),
    });

    let port = start_test_server(config).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/some-route", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Hello"));
}

#[tokio::test]
async fn test_spa_mode_rewrites_all_to_index() {
    let dir = setup_test_dir();
    let mut config = default_config(&dir);
    config.single = true;
    config.rewrites.push(RewriteRule {
        source: "**".to_string(),
        destination: "/index.html".to_string(),
    });

    let port = start_test_server(config).await;
    let client = get_client();

    // In SPA mode, ALL paths (including existing files) rewrite to index.html
    let resp = client
        .get(format!("http://127.0.0.1:{}/app.js", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Hello"));
}

// ============================================================
// Clean URL Tests
// ============================================================

#[tokio::test]
async fn test_clean_urls_redirect_html() {
    let dir = setup_test_dir();
    let mut config = default_config(&dir);
    config.clean_urls = CleanUrlsConfig::Bool(true);

    let port = start_test_server(config).await;
    let client = reqwest::Client::builder()
        .no_gzip()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let resp = client
        .get(format!("http://127.0.0.1:{}/index.html", port))
        .send()
        .await
        .unwrap();

    assert!(resp.status() == 301 || resp.status() == 302);
}

// ============================================================
// Error Page Tests
// ============================================================

#[tokio::test]
async fn test_404_html_error_page() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/not-here", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
    let body = resp.text().await.unwrap();
    assert!(body.contains("404"));
    assert!(body.contains("not found") || body.contains("could not be found"));
}

#[tokio::test]
async fn test_404_json_error() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/not-here", port))
        .header("Accept", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("application/json"));
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["error"]["code"], "not_found");
}

// ============================================================
// Path Traversal Protection Tests
// ============================================================

#[tokio::test]
async fn test_path_traversal_blocked() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;

    // Use raw TCP to avoid reqwest normalizing the ../.. path
    let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .unwrap();

    let request = format!(
        "GET /../../etc/passwd HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        port
    );
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();

    let response_str = String::from_utf8_lossy(&response);
    assert!(response_str.contains("400"), "Expected 400, got: {}", response_str);
}

// ============================================================
// Template Rendering Tests (Unit tests for templates)
// ============================================================

#[test]
fn test_render_directory_template() {
    let files = vec![
        templates::DirEntry {
            base: "app.js".to_string(),
            relative: "/app.js".to_string(),
            title: "app.js (100 B)".to_string(),
            ext: "js".to_string(),
            is_dir: false,
        },
        templates::DirEntry {
            base: "subdir".to_string(),
            relative: "/subdir".to_string(),
            title: "subdir".to_string(),
            ext: "dir".to_string(),
            is_dir: true,
        },
    ];
    let paths = vec![("test".to_string(), "/test".to_string())];

    let html = templates::render_directory("test", &paths, &files);
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Index of"));
    assert!(html.contains("app.js"));
    assert!(html.contains("subdir"));
    assert!(html.contains("folder"));
    assert!(html.contains("file"));
}

#[test]
fn test_render_error_template() {
    let html = templates::render_error(404, "The requested path could not be found");
    assert!(html.contains("404"));
    assert!(html.contains("not found") || html.contains("could not be found"));
    assert!(html.contains("<!DOCTYPE html>"));
}

#[test]
fn test_render_error_json() {
    let json = templates::render_error_json(404, "not_found", "The requested path could not be found");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["error"]["code"], "not_found");
    assert_eq!(parsed["error"]["message"], "The requested path could not be found");
}

// ============================================================
// serve.json Config Tests
// ============================================================

#[test]
fn test_parse_serve_json_config() {
    let json = r#"{
        "public": "build",
        "cleanUrls": true,
        "trailingSlash": true,
        "rewrites": [
            {"source": "/api/**", "destination": "/api/index.html"}
        ],
        "redirects": [
            {"source": "/old", "destination": "/new", "type": 301}
        ],
        "headers": [
            {"source": "**/*.js", "headers": [{"key": "X-Custom", "value": "test"}]}
        ],
        "directoryListing": false,
        "unlisted": [".secret"],
        "renderSingle": true,
        "symlinks": true,
        "etag": false
    }"#;

    let config: config::ServeJsonConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.public, Some("build".to_string()));
    assert!(config.render_single.unwrap());
    assert!(config.symlinks.unwrap());
    assert!(!config.etag.unwrap());
    assert_eq!(config.unlisted.len(), 1);
    assert_eq!(config.rewrites.len(), 1);
    assert_eq!(config.redirects.len(), 1);
    assert_eq!(config.headers.len(), 1);
}

#[test]
fn test_parse_clean_urls_bool() {
    let json = r#"{"cleanUrls": true}"#;
    let config: config::ServeJsonConfig = serde_json::from_str(json).unwrap();
    match config.clean_urls {
        CleanUrlsConfig::Bool(b) => assert!(b),
        _ => panic!("Expected Bool"),
    }
}

#[test]
fn test_parse_clean_urls_patterns() {
    let json = r#"{"cleanUrls": ["/blog/**", "/docs/**"]}"#;
    let config: config::ServeJsonConfig = serde_json::from_str(json).unwrap();
    match config.clean_urls {
        CleanUrlsConfig::Patterns(p) => assert_eq!(p.len(), 2),
        _ => panic!("Expected Patterns"),
    }
}

// ============================================================
// Listen URI Parser Tests
// ============================================================

#[test]
fn test_parse_listen_uri_port_only() {
    let ep = config::parse_listen_uri("8080").unwrap();
    assert_eq!(ep.host, "0.0.0.0");
    assert_eq!(ep.port, 8080);
}

#[test]
fn test_parse_listen_uri_host_port() {
    let ep = config::parse_listen_uri("127.0.0.1:9090").unwrap();
    assert_eq!(ep.host, "127.0.0.1");
    assert_eq!(ep.port, 9090);
}

#[test]
fn test_parse_listen_uri_tcp() {
    let ep = config::parse_listen_uri("tcp://0.0.0.0:5000").unwrap();
    assert_eq!(ep.host, "0.0.0.0");
    assert_eq!(ep.port, 5000);
}

// ============================================================
// CLI Tests
// ============================================================

#[test]
fn test_cli_default_directory() {
    use clap::Parser;
    let args = vaserve::cli::CliArgs::try_parse_from(["serve"]).unwrap();
    assert_eq!(args.directory, ".");
    assert!(!args.single);
    assert!(!args.cors);
}

#[test]
fn test_cli_with_args() {
    use clap::Parser;
    let args = vaserve::cli::CliArgs::try_parse_from([
        "serve", "-s", "-C", "-p", "4000", "build/",
    ])
    .unwrap();
    assert!(args.single);
    assert!(args.cors);
    assert_eq!(args.port, Some(4000));
    assert_eq!(args.directory, "build/");
}

#[test]
fn test_cli_listen_arg() {
    use clap::Parser;
    let args = vaserve::cli::CliArgs::try_parse_from([
        "serve", "-l", "5000",
    ])
    .unwrap();
    assert_eq!(args.listen, vec!["5000"]);
}

#[test]
fn test_cli_no_flags() {
    use clap::Parser;
    let args = vaserve::cli::CliArgs::try_parse_from([
        "serve", "--no-compression", "--no-etag", "--no-port-switching",
        "--no-clipboard", "--no-request-logging",
    ])
    .unwrap();
    assert!(args.no_compression);
    assert!(args.no_etag);
    assert!(args.no_port_switching);
    assert!(args.no_clipboard);
    assert!(args.no_request_logging);
}

#[test]
fn test_cli_debug_and_config() {
    use clap::Parser;
    let args = vaserve::cli::CliArgs::try_parse_from([
        "serve", "-d", "-c", "custom.json",
    ])
    .unwrap();
    assert!(args.debug);
    assert_eq!(args.config, Some("custom.json".to_string()));
}

// ============================================================
// Rewrite Pattern Tests (via integration behavior)
// ============================================================

#[tokio::test]
async fn test_rewrite_in_serve_json() {
    let dir = setup_test_dir();
    let mut config = default_config(&dir);
    config.rewrites.push(RewriteRule {
        source: "/old-path".to_string(),
        destination: "/index.html".to_string(),
    });

    let port = start_test_server(config).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/old-path", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Hello"));
}

// ============================================================
// No ETag Mode Tests
// ============================================================

#[tokio::test]
async fn test_no_etag_sends_last_modified() {
    let dir = setup_test_dir();
    let mut config = default_config(&dir);
    config.no_etag = true;

    let port = start_test_server(config).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/index.html", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().contains_key("last-modified"));
    assert!(!resp.headers().contains_key("etag"));
}

// ============================================================
// URL Decoding Tests
// ============================================================

#[tokio::test]
async fn test_url_encoded_path() {
    let dir = setup_test_dir();
    std::fs::write(dir.join("hello world.txt"), "spaces!").unwrap();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/hello%20world.txt", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body.trim(), "spaces!");
}

// ============================================================
// Subdirectory Tests
// ============================================================

#[tokio::test]
async fn test_serve_subdirectory_file() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/subdir/index.html", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Sub"));
}

// ============================================================
// Query String Tests
// ============================================================

#[tokio::test]
async fn test_ignore_query_string() {
    let dir = setup_test_dir();
    let port = start_test_server(default_config(&dir)).await;
    let client = get_client();

    let resp = client
        .get(format!("http://127.0.0.1:{}/index.html?v=1&t=2", port))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Hello"));
}
