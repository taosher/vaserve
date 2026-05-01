use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;

use crate::cli::CliArgs;
use crate::config;
use crate::handler;

/// Start the HTTP server with port switching logic
pub async fn start(args: CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config = config::load_config(&args);

    // SPA mode tip
    if config.single {
        if let config::CleanUrlsConfig::Bool(false) = config.clean_urls {
            eprintln!("\x1b[36m\x1b[1m INFO \x1b[0m \x1b[36mTip: Use `--clean-urls` in serve.json for SPA-friendly URLs without .html extensions.\x1b[0m");
        }
    }

    // Warn if SSL options are provided but not implemented
    if config.ssl_cert.is_some() || config.ssl_key.is_some() {
        eprintln!("\x1b[33m\x1b[1m WARN \x1b[0m \x1b[33mSSL/TLS support is not yet implemented. Starting HTTP server.\x1b[0m");
    }

    let state = Arc::new(handler::HandlerState::new(config));

    for endpoint in &state.config.endpoints {
        let requested_port = endpoint.port;
        let host = &endpoint.host;

        let actual_port = if state.config.no_port_switching {
            requested_port
        } else {
            find_available_port(host, requested_port).await
        };

        let addr: SocketAddr = format!("{}:{}", host, actual_port).parse()?;
        let app = build_router(state.clone());

        print_startup_message(
            &addr,
            actual_port != requested_port,
            requested_port,
            state.config.no_clipboard,
        );

        let listener = TcpListener::bind(addr).await?;

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
    }

    Ok(())
}

/// Build the axum router with middleware
fn build_router(state: handler::SharedState) -> Router {
    Router::new()
        .fallback(handler::handle_request)
        .with_state(state)
}

/// Find an available port, trying the requested port first
async fn find_available_port(host: &str, port: u16) -> u16 {
    let addr = format!("{}:{}", host, port);
    if TcpListener::bind(&addr).await.is_ok() {
        return port;
    }

    // Try OS-assigned port
    let addr = format!("{}:0", host);
    if let Ok(listener) = TcpListener::bind(&addr).await {
        if let Ok(addr) = listener.local_addr() {
            let assigned = addr.port();
            drop(listener);
            return assigned;
        }
    }

    port
}

/// Print the startup message matching serve's format
fn print_startup_message(addr: &SocketAddr, port_switched: bool, requested_port: u16, no_clipboard: bool) {
    let local_url = format!("http://{}:{}", localhost_display(addr), addr.port());

    let network_ip = local_ip_address::local_ip()
        .map(|ip| format!("http://{}:{}", ip, addr.port()))
        .unwrap_or_default();

    println!();
    println!("   \x1b[32m┌─────────────────────────────────────────────────┐\x1b[0m");
    println!("   \x1b[32m│\x1b[0m                                                 \x1b[32m│\x1b[0m");
    println!("   \x1b[32m│\x1b[0m   Serving!                                      \x1b[32m│\x1b[0m");
    println!("   \x1b[32m│\x1b[0m                                                 \x1b[32m│\x1b[0m");
    println!("   \x1b[32m│\x1b[0m   - Local:    {}", pad_right(&local_url, 43));
    if !network_ip.is_empty() && network_ip != local_url {
        println!("   \x1b[32m│\x1b[0m   - Network:  {}", pad_right(&network_ip, 43));
    }
    println!("   \x1b[32m│\x1b[0m                                                 \x1b[32m│\x1b[0m");

    if port_switched {
        println!(
            "   \x1b[32m│\x1b[0m   \x1b[31mThis port was picked because {} is in use.\x1b[0m",
            requested_port
        );
        println!("   \x1b[32m│\x1b[0m                                                 \x1b[32m│\x1b[0m");
    }

    println!("   \x1b[32m└─────────────────────────────────────────────────┘\x1b[0m");
    println!();

    if !no_clipboard {
        copy_to_clipboard(&local_url);
    }
}

fn localhost_display(addr: &SocketAddr) -> String {
    match addr.ip() {
        std::net::IpAddr::V4(v4) => {
            if v4.is_unspecified() {
                "localhost".to_string()
            } else {
                v4.to_string()
            }
        }
        std::net::IpAddr::V6(v6) => {
            if v6.is_unspecified() {
                "localhost".to_string()
            } else {
                format!("[{}]", v6)
            }
        }
    }
}

fn pad_right(s: &str, width: usize) -> String {
    let visible_len = s.chars().count();
    if visible_len >= width {
        s.to_string()
    } else {
        format!("{}\x1b[0m{}", s, " ".repeat(width.saturating_sub(visible_len)))
    }
}

/// Copy the local URL to clipboard
fn copy_to_clipboard(url: &str) {
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => {
                if clipboard.set_text(url).is_ok() {
                    eprintln!("\x1b[90mCopied local address to clipboard!\x1b[0m");
                }
            }
            Err(_) => {
                // Clipboard not available (headless), silently ignore
            }
        }
    }
}
