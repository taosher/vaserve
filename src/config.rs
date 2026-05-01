use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::cli::CliArgs;

/// Configuration loaded from serve.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct ServeJsonConfig {
    /// Path to the directory to serve
    pub public: Option<String>,

    /// Clean URLs: strip .html or /index
    #[serde(default)]
    pub clean_urls: CleanUrlsConfig,

    /// URL rewriting rules
    #[serde(default)]
    pub rewrites: Vec<RewriteRule>,

    /// HTTP redirect rules
    #[serde(default)]
    pub redirects: Vec<RedirectRule>,

    /// Custom HTTP headers per route
    #[serde(default)]
    pub headers: Vec<HeaderRule>,

    /// Enable/disable directory listings
    #[serde(default)]
    pub directory_listing: DirectoryListingConfig,

    /// Paths hidden from directory listings
    #[serde(default)]
    pub unlisted: Vec<String>,

    /// Control trailing slash behavior
    pub trailing_slash: Option<bool>,

    /// SPA mode: rewrite not-found to index.html
    pub render_single: Option<bool>,

    /// Whether to follow symbolic links
    pub symlinks: Option<bool>,

    /// Whether to generate ETags
    pub etag: Option<bool>,
}

/// Clean URLs config: can be boolean or array of patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CleanUrlsConfig {
    Bool(bool),
    Patterns(Vec<String>),
}

impl Default for CleanUrlsConfig {
    fn default() -> Self {
        CleanUrlsConfig::Bool(false)
    }
}

/// Directory listing config: can be boolean or array of patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DirectoryListingConfig {
    Bool(bool),
    Patterns(Vec<String>),
}

impl Default for DirectoryListingConfig {
    fn default() -> Self {
        DirectoryListingConfig::Bool(true)
    }
}

/// URL rewrite rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteRule {
    pub source: String,
    pub destination: String,
}

/// HTTP redirect rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedirectRule {
    pub source: String,
    pub destination: String,
    #[serde(rename = "type", default = "default_redirect_type")]
    pub status_type: u16,
}

fn default_redirect_type() -> u16 {
    301
}

/// Custom header rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderRule {
    pub source: String,
    pub headers: Vec<HeaderEntry>,
}

/// Key-value header entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderEntry {
    pub key: String,
    pub value: String,
}

/// The final merged application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Directory to serve
    pub public: String,

    /// Listen endpoints (host:port)
    pub endpoints: Vec<EndpointConfig>,

    /// SPA mode (rewrite all not-found to index.html)
    pub single: bool,

    /// Show debug info
    pub debug: bool,

    /// Disable request logging
    pub no_request_logging: bool,

    /// Enable CORS
    pub cors: bool,

    /// Copy local address to clipboard
    pub no_clipboard: bool,

    /// Disable gzip compression
    pub no_compression: bool,

    /// Send Last-Modified instead of ETag
    pub no_etag: bool,

    /// Resolve symlinks
    pub symlinks: bool,

    /// SSL certificate path
    pub ssl_cert: Option<String>,

    /// SSL private key path
    pub ssl_key: Option<String>,

    /// SSL passphrase path
    pub ssl_pass: Option<String>,

    /// Don't auto-switch ports
    pub no_port_switching: bool,

    /// Clean URLs config
    pub clean_urls: CleanUrlsConfig,

    /// URL rewrites
    pub rewrites: Vec<RewriteRule>,

    /// HTTP redirects
    pub redirects: Vec<RedirectRule>,

    /// Custom headers per route
    pub custom_headers: Vec<HeaderRule>,

    /// Directory listing config
    pub directory_listing: DirectoryListingConfig,

    /// Unlisted paths
    pub unlisted: Vec<String>,

    /// Trailing slash behavior
    pub trailing_slash: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct EndpointConfig {
    pub host: String,
    pub port: u16,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        EndpointConfig {
            host: "0.0.0.0".to_string(),
            port: 3000,
        }
    }
}

/// Parse a listen URI into an EndpointConfig
pub fn parse_listen_uri(uri: &str) -> Option<EndpointConfig> {
    // Handle simple port number
    if let Ok(port) = uri.parse::<u16>() {
        return Some(EndpointConfig {
            host: "0.0.0.0".to_string(),
            port,
        });
    }

    // Handle tcp://host:port
    if let Some(stripped) = uri.strip_prefix("tcp://") {
        let parts: Vec<&str> = stripped.rsplitn(2, ':').collect();
        let port = parts[0].parse::<u16>().ok()?;
        let host = if parts.len() > 1 { parts[1].to_string() } else { "0.0.0.0".to_string() };
        let host = if host.is_empty() { "0.0.0.0".to_string() } else { host };
        return Some(EndpointConfig { host, port });
    }

    // Handle host:port
    let parts: Vec<&str> = uri.rsplitn(2, ':').collect();
    if parts.len() == 2 {
        if let Ok(port) = parts[0].parse::<u16>() {
            return Some(EndpointConfig {
                host: parts[1].to_string(),
                port,
            });
        }
    }

    None
}

/// Load and merge configuration from CLI args and serve.json
pub fn load_config(args: &CliArgs) -> AppConfig {
    let mut config = AppConfig {
        public: args.directory.clone(),
        endpoints: vec![],
        single: args.single,
        debug: args.debug,
        no_request_logging: args.no_request_logging,
        cors: args.cors,
        no_clipboard: args.no_clipboard,
        no_compression: args.no_compression,
        no_etag: args.no_etag,
        symlinks: args.symlinks,
        ssl_cert: args.ssl_cert.clone(),
        ssl_key: args.ssl_key.clone(),
        ssl_pass: args.ssl_pass.clone(),
        no_port_switching: args.no_port_switching,
        clean_urls: CleanUrlsConfig::Bool(false),
        rewrites: vec![],
        redirects: vec![],
        custom_headers: vec![],
        directory_listing: DirectoryListingConfig::Bool(true),
        unlisted: vec![],
        trailing_slash: None,
    };

    // Build endpoints from --listen or -p
    if !args.listen.is_empty() {
        config.endpoints = args
            .listen
            .iter()
            .filter_map(|uri| parse_listen_uri(uri))
            .collect();
    } else if let Some(port) = args.port {
        config.endpoints.push(EndpointConfig {
            host: "0.0.0.0".to_string(),
            port,
        });
    }

    // Default endpoint
    if config.endpoints.is_empty() {
        config.endpoints.push(EndpointConfig::default());
    }

    // Load serve.json from the public directory
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| Path::new(&config.public).join("serve.json").to_string_lossy().to_string());

    if let Ok(contents) = std::fs::read_to_string(&config_path) {
        if let Ok(serve_config) = serde_json::from_str::<ServeJsonConfig>(&contents) {
            // Merge serve.json into config
            if let Some(public) = serve_config.public {
                config.public = public;
            }
            if serve_config.render_single.unwrap_or(false) {
                config.single = true;
            }
            if let Some(symlinks) = serve_config.symlinks {
                config.symlinks = symlinks;
            }
            if let Some(etag) = serve_config.etag {
                config.no_etag = !etag;
            }
            if let Some(ts) = serve_config.trailing_slash {
                config.trailing_slash = Some(ts);
            }

            config.clean_urls = serve_config.clean_urls;
            config.unlisted = serve_config.unlisted;
            config.directory_listing = serve_config.directory_listing;

            // Merge rewrites: config rewrites first, then CLI rewrites (for SPA)
            let mut all_rewrites = serve_config.rewrites;
            // SPA mode adds a catch-all rewrite
            if config.single {
                all_rewrites.insert(
                    0,
                    RewriteRule {
                        source: "**".to_string(),
                        destination: "/index.html".to_string(),
                    },
                );
            }
            config.rewrites = all_rewrites;

            config.redirects = serve_config.redirects;
            config.custom_headers = serve_config.headers;
        } else if config.debug {
            eprintln!("WARN: Failed to parse {}", config_path);
        }
    } else if config.debug {
        // Only warn if a custom config path was specified but not found
        if args.config.is_some() {
            eprintln!("WARN: Config file not found: {}", config_path);
        }
    }

    // If SPA mode was enabled via CLI but no rewrites exist yet, add the catch-all
    if config.single && config.rewrites.is_empty() {
        config.rewrites.push(RewriteRule {
            source: "**".to_string(),
            destination: "/index.html".to_string(),
        });
    }

    config
}
