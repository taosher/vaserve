use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::Response;
use bytes::Bytes;
use sha1::{Digest, Sha1};

use crate::config::{AppConfig, CleanUrlsConfig, DirectoryListingConfig};
use crate::templates;

/// Shared handler state
pub struct HandlerState {
    pub config: AppConfig,
    /// Public directory absolute path
    pub public_path: PathBuf,
    /// ETag cache: absolute_path -> (mtime, etag)
    pub etag_cache: tokio::sync::RwLock<HashMap<PathBuf, (u64, String)>>,
}

impl HandlerState {
    pub fn new(config: AppConfig) -> Self {
        let public_path = Path::new(&config.public)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&config.public));

        HandlerState {
            config,
            public_path,
            etag_cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

pub type SharedState = Arc<HandlerState>;

/// Main request handler implementing serve-handler logic
pub async fn handle_request(
    State(state): State<SharedState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    req: Request,
) -> Response {
    let method = req.method().clone();
    let uri_path = req.uri().path().to_string();
    let accepts_json = accepts_json(req.headers());
    let range_header = req.headers().get(header::RANGE).cloned();

    // Log the request
    if !state.config.no_request_logging {
        log_request(&method, &uri_path, addr.ip());
    }

    // Handle the request
    match serve_path(&state, &uri_path, accepts_json, &range_header).await {
        Ok(response) => response,
        Err(status) => {
            if !state.config.no_request_logging {
                log_response(status, 0);
            }
            error_response(status, accepts_json)
        }
    }
}

/// Core serve logic: resolve a URL path to a file and serve it
async fn serve_path(
    state: &SharedState,
    path: &str,
    accepts_json: bool,
    range_header: &Option<HeaderValue>,
) -> Result<Response, StatusCode> {
    // Decode URL
    let decoded = decode_uri_path(path)?;

    // Handle trailing slash - may produce a redirect
    if let Some(trailing_result) = handle_trailing_slash(state, &decoded) {
        if trailing_result != decoded {
            return Ok(redirect_response(&trailing_result, 301));
        }
    }
    let decoded = handle_trailing_slash(state, &decoded).unwrap_or(decoded);

    // Apply rewrite rules
    let decoded = apply_rewrites(state, &decoded);

    // Handle redirects
    if let Some(redirect) = check_redirects(state, &decoded) {
        return Ok(redirect_response(&redirect.0, redirect.1));
    }

    // Handle clean URLs
    if let Some(redirect) = check_clean_urls(state, &decoded) {
        return Ok(redirect_response(&redirect, 301));
    }

    // Resolve the file path
    let (abs_path, is_dir) = resolve_path(state, &decoded)?;

    // Handle directory
    if is_dir {
        return serve_directory(state, &abs_path, &decoded, accepts_json).await;
    }

    // Serve the file
    serve_file(state, &abs_path, accepts_json, range_header).await
}

/// Decode URI path component
fn decode_uri_path(path: &str) -> Result<String, StatusCode> {
    // Remove query string
    let clean = path.split('?').next().unwrap_or(path);

    // Decode each segment
    let decoded = clean
        .split('/')
        .map(|segment| {
            urlencoding(segment)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .join("/");

    Ok(decoded)
}

/// Simple URL decoding (percent-encoded sequences)
fn urlencoding(input: &str) -> Result<String, ()> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().ok_or(())?;
            let h2 = chars.next().ok_or(())?;
            let byte = u8::from_str_radix(&format!("{}{}", h1, h2), 16).map_err(|_| ())?;
            result.push(byte as char);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

/// Handle trailing slash behavior - returns None if no redirect needed,
/// or Some(redirect_path) if a redirect should be performed
fn handle_trailing_slash(state: &SharedState, path: &str) -> Option<String> {
    // Collapse double slashes first
    let path = collapse_double_slashes(path);

    match state.config.trailing_slash {
        Some(true) => {
            // Ensure trailing slash for paths without extensions
            if !path.ends_with('/')
                && !path.starts_with('.')
                && !has_extension(&path)
            {
                Some(format!("{}/", path))
            } else {
                Some(path)
            }
        }
        Some(false) => {
            // Strip trailing slash
            if path.len() > 1 && path.ends_with('/') {
                Some(path.trim_end_matches('/').to_string())
            } else {
                Some(path)
            }
        }
        None => Some(path),
    }
}

fn collapse_double_slashes(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut prev_was_slash = false;
    for c in path.chars() {
        if c == '/' {
            if !prev_was_slash {
                result.push(c);
            }
            prev_was_slash = true;
        } else {
            result.push(c);
            prev_was_slash = false;
        }
    }
    result
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .map(|segment| segment.contains('.') && !segment.starts_with('.'))
        .unwrap_or(false)
}

/// Check and handle clean URLs
fn check_clean_urls(state: &SharedState, path: &str) -> Option<String> {
    if !applicable_clean_urls(path, &state.config.clean_urls) {
        return None;
    }

    // Strip .html suffix
    if let Some(stripped) = path.strip_suffix(".html") {
        return Some(stripped.to_string());
    }

    // Strip /index suffix
    if let Some(stripped) = path.strip_suffix("/index") {
        return Some(format!("{}/", stripped));
    }

    None
}

fn applicable_clean_urls(path: &str, config: &CleanUrlsConfig) -> bool {
    match config {
        CleanUrlsConfig::Bool(b) => *b,
        CleanUrlsConfig::Patterns(patterns) => patterns.iter().any(|p| path_matches(path, p)),
    }
}

/// Check redirect rules
fn check_redirects(state: &SharedState, path: &str) -> Option<(String, u16)> {
    for rule in &state.config.redirects {
        if let Some(target) = match_rewrite(path, &rule.source, &rule.destination) {
            return Some((target, rule.status_type));
        }
    }
    None
}

/// Apply rewrite rules (recursive)
fn apply_rewrites(state: &SharedState, path: &str) -> String {
    let mut current = path.to_string();
    let mut used: Vec<usize> = Vec::new();

    loop {
        let mut matched = false;
        for (idx, rule) in state.config.rewrites.iter().enumerate() {
            if used.contains(&idx) {
                continue;
            }
            if let Some(target) = match_rewrite(&current, &rule.source, &rule.destination) {
                used.push(idx);
                current = target;
                matched = true;
                break;
            }
        }
        if !matched {
            break;
        }
    }

    current
}

/// Match a path against a rewrite pattern and produce a target
fn match_rewrite(path: &str, source: &str, destination: &str) -> Option<String> {
    // Simple glob matching
    if source == "**" {
        return Some(destination.to_string());
    }

    if source == "*" {
        return Some(destination.to_string());
    }

    // Exact match
    if source == path {
        return Some(destination.to_string());
    }

    // Source with ** suffix: prefix match
    if let Some(prefix) = source.strip_suffix("/**") {
        if path.starts_with(prefix) {
            let remainder = &path[prefix.len()..];
            let target = destination.trim_end_matches('/');
            return Some(format!("{}{}", target, remainder));
        }
    }

    // Source with * wildcard
    if source.contains('*') {
        let parts: Vec<&str> = source.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            if path.starts_with(prefix) && path.ends_with(suffix) && path.len() >= prefix.len() + suffix.len() {
                let captured = &path[prefix.len()..path.len() - suffix.len()];
                if !captured.contains('/') {
                    let target = destination.replace("$1", captured);
                    return Some(target);
                }
            }
        }
    }

    // Source with :param (path parameter)
    if source.contains(':') {
        // Split both source and path into segments
        let src_segments: Vec<&str> = source.split('/').filter(|s| !s.is_empty()).collect();
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if src_segments.len() != path_segments.len() {
            return None;
        }

        let mut params: HashMap<String, String> = HashMap::new();
        for (s, p) in src_segments.iter().zip(path_segments.iter()) {
            if let Some(name) = s.strip_prefix(':') {
                params.insert(name.to_string(), p.to_string());
            } else if s != p && *s != "**" && *s != "*" {
                return None;
            }
        }

        let mut target = destination.to_string();
        if target.contains(':') {
            // If destination is a URL path, just use it directly
            // (e.g., /api/:id -> /api/data)
            for (name, value) in &params {
                target = target.replace(&format!(":{}", name), value);
            }
        }
        return Some(target);
    }

    None
}

/// Simple path matching (for clean URLs, directory listing patterns)
fn path_matches(path: &str, pattern: &str) -> bool {
    if pattern == "**" || pattern == "*" {
        return true;
    }
    path == pattern
}

/// Resolve a URL path to a filesystem path
fn resolve_path(state: &SharedState, path: &str) -> Result<(PathBuf, bool), StatusCode> {
    let relative = path.trim_start_matches('/');

    // Check for path traversal (.. components)
    if relative.split('/').any(|s| s == "..") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let abs_path = state.public_path.join(relative);

    // Canonicalize the path
    let abs_path = abs_path.canonicalize().unwrap_or_else(|_| abs_path.clone());

    // Check path traversal (double check with canonicalized path)
    if !abs_path.starts_with(&state.public_path) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if it's a symlink
    let metadata = std::fs::symlink_metadata(&abs_path).map_err(|_| {
        // File not found
        StatusCode::NOT_FOUND
    })?;

    let is_symlink = metadata.file_type().is_symlink();

    if is_symlink && !state.config.symlinks {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get real path and metadata
    let real_path = if is_symlink {
        abs_path.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?
    } else {
        abs_path.clone()
    };

    let metadata = std::fs::metadata(&real_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let is_dir = metadata.is_dir();

    Ok((real_path, is_dir))
}

/// Serve a directory listing
async fn serve_directory(
    state: &SharedState,
    abs_path: &Path,
    url_path: &str,
    accepts_json: bool,
) -> Result<Response, StatusCode> {
    // Check directory listing config
    if !is_directory_listing_allowed(url_path, &state.config.directory_listing) {
        // Try serving index.html instead
        let index_path = abs_path.join("index.html");
        if index_path.exists() {
            return serve_file(
                state,
                &index_path,
                accepts_json,
                &None,
            ).await;
        }
        return Err(StatusCode::NOT_FOUND);
    }

    // Read directory entries
    let entries = std::fs::read_dir(abs_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut files: Vec<templates::DirEntry> = Vec::new();
    let default_unlisted = vec![".DS_Store".to_string(), ".git".to_string()];

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip unlisted files
        if default_unlisted.contains(&name) || state.config.unlisted.contains(&name) {
            continue;
        }

        let file_type = entry.file_type().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let is_dir = file_type.is_dir();

        let relative = if url_path.ends_with('/') {
            format!("{}/{}", url_path.trim_end_matches('/'), name)
        } else {
            format!("{}/{}", url_path, name)
        };

        let (ext, title) = if is_dir {
            ("dir".to_string(), name.clone())
        } else {
            let ext = name
                .rsplit('.')
                .next()
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            let size = entry
                .metadata()
                .ok()
                .map(|m| m.len())
                .unwrap_or(0);
            let title = format!("{} (&nbsp;{}&nbsp;)", name, bytesize::to_string(size, false));
            (ext, title)
        };

        files.push(templates::DirEntry {
            base: name,
            relative,
            title,
            ext,
            is_dir,
        });
    }

    // Sort: directories first, then alphabetically
    files.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.base.to_lowercase().cmp(&b.base.to_lowercase()))
    });

    // Add parent directory link if not at root
    let directory_name = url_path.to_string();
    let mut paths: Vec<(String, String)> = Vec::new();

    // Build breadcrumbs
    let segments: Vec<&str> = url_path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    let mut accumulated = String::new();
    for segment in &segments {
        accumulated.push('/');
        accumulated.push_str(segment);
        paths.push((segment.to_string(), accumulated.clone()));
    }

    if accepts_json {
        // Return JSON
        let json = serde_json::json!({
            "files": files.iter().map(|f| {
                serde_json::json!({
                    "base": f.base,
                    "relative": f.relative,
                    "type": if f.is_dir { "folder" } else { "file" },
                    "ext": f.ext,
                    "title": f.title,
                })
            }).collect::<Vec<_>>()
        });
        let body = serde_json::to_string_pretty(&json).unwrap_or_default();
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(body))
            .unwrap());
    }

    // Render HTML
    let html = templates::render_directory(&directory_name, &paths, &files);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap())
}

fn is_directory_listing_allowed(path: &str, config: &DirectoryListingConfig) -> bool {
    match config {
        DirectoryListingConfig::Bool(b) => *b,
        DirectoryListingConfig::Patterns(patterns) => {
            if patterns.is_empty() {
                return false;
            }
            patterns.iter().any(|p| path_matches(path, p))
        }
    }
}

/// Serve a file from disk
async fn serve_file(
    state: &SharedState,
    abs_path: &Path,
    _accepts_json: bool,
    range_header: &Option<HeaderValue>,
) -> Result<Response, StatusCode> {
    let metadata = std::fs::metadata(abs_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let file_size = metadata.len();
    let mtime = metadata
        .modified()
        .unwrap_or(SystemTime::now())
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Read file content
    let content = tokio::fs::read(abs_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get MIME type
    let mime = mime_guess::from_path(abs_path)
        .first_or_octet_stream();
    let content_type = mime.to_string();

    // ETag or Last-Modified
    let etag = if !state.config.no_etag {
        Some(compute_etag(state, abs_path, mtime, &content).await)
    } else {
        None
    };

    // Check If-None-Match for 304
    if let Some(ref _etag_val) = etag {
        // We'd need request headers to check, but in our simplified handler
        // we don't pass them here. The main handler should handle this.
    }

    // Handle Range requests
    if let Some(range_val) = range_header {
        if let Ok(range_str) = range_val.to_str() {
            if let Some((start, end)) = parse_range(range_str, file_size) {
                let slice = &content[start as usize..=end as usize];
                let content_length = slice.len() as u64;

                let mut builder = Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, &content_type)
                    .header(header::CONTENT_LENGTH, content_length)
                    .header(
                        header::CONTENT_RANGE,
                        format!("bytes {}-{}/{}", start, end, file_size),
                    )
                    .header(
                        header::CONTENT_DISPOSITION,
                        format!("inline; filename=\"{}\"", abs_path.file_name().unwrap_or_default().to_string_lossy()),
                    );

                if let Some(ref tag) = etag {
                    builder = builder.header(header::ETAG, tag.as_str());
                } else {
                    let lm = httpdate(mtime);
                    builder = builder.header(header::LAST_MODIFIED, lm);
                }

                return Ok(builder.body(Body::from(Bytes::copy_from_slice(slice))).unwrap());
            } else {
                // Invalid range - return 416 with Content-Range
                let builder = Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{}", file_size));

                return Ok(builder.body(Body::empty()).unwrap());
            }
        }
    }

    // Normal 200 response
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &content_type)
        .header(header::CONTENT_LENGTH, file_size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{}\"", abs_path.file_name().unwrap_or_default().to_string_lossy()),
        )
        .header(header::ACCEPT_RANGES, "bytes");

    if let Some(ref tag) = etag {
        builder = builder.header(header::ETAG, tag.as_str());
    } else {
        let lm = httpdate(mtime);
        builder = builder.header(header::LAST_MODIFIED, lm);
    }

    // Add custom headers matching this path
    let url_path = abs_path
        .strip_prefix(&state.public_path)
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .to_string();
    let url_path = format!("/{}", url_path.trim_start_matches('/'));

    for header_rule in &state.config.custom_headers {
        if path_matches(&url_path, &header_rule.source) {
            for entry in &header_rule.headers {
                if let Ok(val) = HeaderValue::from_str(&entry.value) {
                    builder = builder.header(&entry.key, val);
                }
            }
        }
    }

    Ok(builder.body(Body::from(content)).unwrap())
}

/// Parse a Range header, returns (start, end) inclusive
fn parse_range(range: &str, file_size: u64) -> Option<(u64, u64)> {
    if file_size == 0 {
        return None;
    }

    let range = range.strip_prefix("bytes=")?;
    let parts: Vec<&str> = range.split('-').collect();
    if parts.len() != 2 {
        return None;
    }

    if parts[0].is_empty() && !parts[1].is_empty() {
        // suffix range: bytes=-500
        let suffix: u64 = parts[1].parse().ok()?;
        if suffix > file_size {
            return Some((0, file_size - 1));
        }
        return Some((file_size - suffix, file_size - 1));
    }

    if parts[1].is_empty() {
        // open-ended: bytes=500-
        let start: u64 = parts[0].parse().ok()?;
        if start >= file_size {
            return None;
        }
        return Some((start, file_size - 1));
    }

    let start: u64 = parts[0].parse().ok()?;
    let end: u64 = parts[1].parse().ok()?;

    if start > end || start >= file_size {
        return None;
    }

    let end = end.min(file_size - 1);
    Some((start, end))
}

/// Compute ETag for a file
async fn compute_etag(
    state: &SharedState,
    abs_path: &Path,
    mtime: u64,
    content: &[u8],
) -> String {
    // Check cache
    {
        let cache = state.etag_cache.read().await;
        if let Some((cached_mtime, cached_etag)) = cache.get(abs_path) {
            if *cached_mtime == mtime {
                return cached_etag.clone();
            }
        }
    }

    // Compute new ETag
    let ext = abs_path
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut hasher = Sha1::new();
    hasher.update(ext.as_bytes());
    hasher.update(content);
    let hash = format!("{:x}", hasher.finalize());
    let etag = format!("\"{}\"", &hash[..27]);

    // Update cache
    {
        let mut cache = state.etag_cache.write().await;
        cache.insert(abs_path.to_path_buf(), (mtime, etag.clone()));
    }

    etag
}

/// Format mtime as HTTP date
fn httpdate(ts: u64) -> String {
    use std::time::Duration;
    let d = UNIX_EPOCH + Duration::from_secs(ts);
    let secs = d.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    // Simple formatting
    let _days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // This is a simplified version. For exact behavior, we'd need proper date lib.
    // Using a simple approximation based on Unix epoch (Thursday, Jan 1, 1970)
    let weekdays = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    // Calculate year, month, day from days since epoch
    let mut remaining_days = _days as i64;
    let mut year = 1970i64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let month_lengths = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0usize;
    for (i, &ml) in month_lengths.iter().enumerate() {
        if remaining_days < ml as i64 {
            month = i;
            break;
        }
        remaining_days -= ml as i64;
    }

    let day = remaining_days + 1;
    let weekday_idx = (_days + 4) % 7; // Jan 1, 1970 was Thursday

    format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
        weekdays[weekday_idx as usize],
        day,
        months[month],
        year,
        hours,
        minutes,
        seconds
    )
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Log an incoming HTTP request
fn log_request(method: &Method, path: &str, ip: std::net::IpAddr) {
    let ts = chrono_now();
    eprintln!("\x1b[44m\x1b[1m HTTP \x1b[0m {} {} {} {}", ts, ip, method, path);
}

fn log_response(status: StatusCode, _duration_ms: u64) {
    let code = status.as_u16();
    let color = if code < 400 { "\x1b[32m" } else { "\x1b[31m" };
    eprintln!("{}  {} \x1b[0m", color, code);
}

/// Simple timestamp for logging
fn chrono_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let _days = secs / 86400;
    let tod = secs % 86400;
    let h = tod / 3600;
    let m = (tod % 3600) / 60;
    let s = tod % 60;

    // Simple date from epoch (doesn't need to be exact for logging)
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Check if request accepts JSON
fn accepts_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/json"))
        .unwrap_or(false)
}

/// Generate an error response (HTML or JSON)
fn error_response(status: StatusCode, accepts_json: bool) -> Response {
    let code = status.as_u16();
    let (error_code, message) = match code {
        400 => ("bad_request", "Bad Request"),
        404 => ("not_found", "The requested path could not be found"),
        500 | _ => ("internal_server_error", "A server error has occurred"),
    };

    if accepts_json {
        let json = templates::render_error_json(code, error_code, message);
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
            .body(Body::from(json))
            .unwrap()
    } else {
        let html = templates::render_error(code, message);
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(html))
            .unwrap()
    }
}

/// Generate a redirect response
fn redirect_response(location: &str, status: u16) -> Response {
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::MOVED_PERMANENTLY);
    Response::builder()
        .status(code)
        .header(header::LOCATION, location)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(format!(
            "Redirecting to <a href=\"{}\">{}</a>",
            location, location
        )))
        .unwrap()
}
