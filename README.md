# vaserve

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**vaserve 是 [vercel/serve](https://github.com/vercel/serve) 的 Rust 版本。**
Static file serving and directory listing — API-compatible with all CLI arguments and configuration of the original vercel/serve.

## User Story

**As a developer** working on static websites, single-page applications, or any project with static assets, **I want** a zero-configuration, instant-on static file server **so that** I can quickly preview my work, share it on the local network, and iterate without needing to install or configure Node.js, nginx, Apache, or any heavy web server infrastructure.

### What Problems Does serve Solve?

1. **"I just want to see what my build output looks like"**
   You've run `npm run build` and have a `dist/` folder. Opening `index.html` directly in the browser breaks — relative paths fail, API calls to `/api` return nothing, and routing relies on an HTTP server context. `vaserve dist/` gives you a fully working server in one command.

2. **"I need to test my SPA routing"**
   Your React/Vue/Svelte app uses client-side routing (`/about`, `/dashboard`). Direct file access can only serve `index.html`. With `vaserve -s dist/`, every not-found route rewrites to `index.html` — just like production.

3. **"I want to share files on my local network"**
   You have screenshots, documents, or build artifacts to share with teammates. `vaserve` starts in seconds, copies the URL to your clipboard, and with `-C` enables CORS so your colleagues can access everything with zero config.

4. **"I want to avoid Node.js dependency overhead"**
   The original `serve` pulls in Node.js, npm, and hundreds of transient dependencies. You're working in a Rust ecosystem — or you simply want a single, tiny binary. `vaserve` is a drop-in replacement: same CLI, same `serve.json` config, same behavior. Substitute `npx serve` with `vaserve`.

5. **"I need a static server in CI/CD or a Docker container"**
   A single statically-linked binary with no runtime dependencies and a tiny footprint. `vaserve dist/` works anywhere Rust compiles — Linux, macOS, Windows, ARM. Deploy it in a `FROM scratch` Docker image, drop it into a CI pipeline, or run it on a Raspberry Pi.

## Installation

### From crates.io

```bash
cargo install vaserve
```

### From Source

```bash
git clone https://github.com/taosher/vaserve.git
cd vaserve
cargo build --release
```

The binary will be at `target/release/vaserve`. Copy it to a directory in your `$PATH`:

```bash
cp target/release/vaserve /usr/local/bin/
```

### Requirements

- Rust 1.70+ (MSRV)

## Quick Start

```bash
# Serve the current directory on port 3000
vaserve

# Serve a specific folder
vaserve build/

# Custom port
vaserve -l 8080

# SPA mode (rewrite all routes to index.html)
vaserve -s dist/

# With CORS
vaserve -C

# Multiple endpoints
vaserve -l 3000 -l 3001
```

## Usage

```
$ vaserve --help
$ vaserve --version
$ vaserve folder_name
$ vaserve [-l listen_uri [-l ...]] [directory]
```

By default, vaserve listens on `0.0.0.0:3000` and serves the current working directory.

## CLI Options

| Option | Alias | Description | Default |
|--------|-------|-------------|---------|
| `--help` | `-h` | Show help message | — |
| `--version` | `-v` | Display version | — |
| `--listen <uri>` | `-l` | Listen URI (repeatable) | `0.0.0.0:3000` |
| `-p <port>` |  | Custom port (deprecated, use `-l`) | — |
| `--single` | `-s` | SPA mode: rewrite all not-found to `index.html` | `false` |
| `--debug` | `-d` | Show debug information | `false` |
| `--config <path>` | `-c` | Custom path to `serve.json` | `serve.json` |
| `--no-request-logging` | `-L` | Disable request logging | `false` |
| `--cors` | `-C` | Enable CORS (`Access-Control-Allow-Origin: *`) | `false` |
| `--no-clipboard` | `-n` | Don't copy address to clipboard | `false` |
| `--no-compression` | `-u` | Disable gzip compression | `false` |
| `--no-etag` |  | Send `Last-Modified` instead of `ETag` | `false` |
| `--symlinks` | `-S` | Resolve symlinks instead of 404 | `false` |
| `--ssl-cert <path>` |  | Path to SSL certificate (PEM/PKCS12) | — |
| `--ssl-key <path>` |  | Path to SSL private key | — |
| `--ssl-pass <path>` |  | Path to SSL passphrase file | — |
| `--no-port-switching` |  | Don't auto-switch port if occupied | `false` |

### Listen Endpoints

```bash
# Simple port (defaults to 0.0.0.0)
vaserve -l 1234

# TCP with host
vaserve -l tcp://hostname:1234

# Host:port
vaserve -l 127.0.0.1:3000

# Multiple endpoints
vaserve -l tcp://0.0.0.0:3000 -l tcp://0.0.0.0:3001
```

## serve.json Configuration

Create a `serve.json` file in your public directory to declaratively configure behavior:

```json
{
  "public": "dist",
  "cleanUrls": true,
  "trailingSlash": false,
  "rewrites": [
    { "source": "/api/**", "destination": "/api/index.html" }
  ],
  "redirects": [
    { "source": "/old-blog", "destination": "/blog", "type": 301 }
  ],
  "headers": [
    {
      "source": "**/*.js",
      "headers": [
        { "key": "X-Custom-Header", "value": "custom-value" }
      ]
    }
  ],
  "directoryListing": false,
  "unlisted": [".secret", "private"],
  "renderSingle": true,
  "symlinks": false,
  "etag": true
}
```

### Configuration Reference

| Key | Type | Description |
|-----|------|-------------|
| `public` | `string` | Directory path to serve |
| `cleanUrls` | `boolean \| string[]` | Strip `.html`/`/index` from URLs |
| `trailingSlash` | `boolean` | Force add/remove trailing slashes |
| `rewrites` | `{source, destination}[]` | URL rewrite rules |
| `redirects` | `{source, destination, type}[]` | HTTP redirect rules (default: 301) |
| `headers` | `{source, headers[]}[]` | Custom HTTP headers per route |
| `directoryListing` | `boolean \| string[]` | Enable/disable directory listing |
| `unlisted` | `string[]` | Paths hidden from directory listing |
| `renderSingle` | `boolean` | SPA mode (rewrite 404s to `index.html`) |
| `symlinks` | `boolean` | Follow symbolic links |
| `etag` | `boolean` | Generate ETag headers for caching |

### Rewrite Pattern Syntax

| Pattern | Example | Description |
|---------|---------|-------------|
| `**` | `**` | Match all paths |
| `/prefix/**` | `/api/**` | Match paths under a prefix |
| `*` wildcard | `*.php` | Match within a single segment |
| `:param` | `/user/:id` | Named path parameter |

## Features

### File Serving

- Correct `Content-Type` headers via file extension detection
- `Content-Disposition: inline` for all files
- `Accept-Ranges: bytes` for partial content support
- Automatic gzip compression (disable with `--no-compression`)

### ETag Caching

SHA-1 based ETags, cached per-file with mtime-based invalidation. Disable with `--no-etag` to use `Last-Modified` instead.

### Range Requests

Full support for HTTP byte range requests:

```bash
curl -H "Range: bytes=0-1023" http://localhost:3000/large-file.bin
curl -H "Range: bytes=-500" http://localhost:3000/large-file.bin
```

Invalid ranges return `416 Range Not Satisfiable`.

### Directory Listing

Clean, responsive HTML directory listings matching serve's design:
- Directories listed before files, alphabetical within each group
- Breadcrumb navigation
- Folder and file icons with SVG CSS backgrounds
- JSON output when `Accept: application/json` is set
- Hidden files filtered (`.DS_Store`, `.git`, custom `unlisted` patterns)

### Error Pages

- **400 Bad Request** — path traversal or malformed URLs
- **404 Not Found** — missing files, disabled symlinks
- **500 Internal Server Error** — unhandled exceptions
- HTML output (matching serve's design) or JSON (`Accept: application/json`)
- Custom error pages supported: place `404.html` in your public directory

### Port Switching

When a requested port is occupied, serve automatically selects an available port. Disable with `--no-port-switching`.

### Clipboard

On start, the local URL is automatically copied to the clipboard. Disable with `--no-clipboard`.

### SPA Mode

`--single` / `-s` enables Single Page Application mode. All not-found requests are rewritten to `index.html`, allowing client-side routing to work correctly.

## Architecture

```
src/
├── main.rs       # Entry point — CLI and server orchestration
├── lib.rs        # Library root (exposes modules for testing)
├── cli.rs        # CLI argument parsing via clap (derive)
├── config.rs     # serve.json deserialization + CLI merge
├── server.rs     # HTTP server, port switching, startup messages
├── handler.rs    # Core request handler (serve-handler logic)
└── templates.rs  # Directory listing & error page HTML
```

### Technology Stack

| Component | Crate |
|-----------|-------|
| HTTP server | `axum` 0.7 + `hyper` 1 |
| Async runtime | `tokio` |
| CLI parsing | `clap` 4 (derive) |
| JSON config | `serde` + `serde_json` |
| MIME types | `mime_guess` |
| ETags | `sha1` |
| Clipboard | `arboard` |
| Gzip | `flate2` |

## Development

### Build

```bash
cargo build
cargo build --release
```

### Run

```bash
cargo run -- -l 3000 ./public
```

### Test

```bash
cargo test
cargo test -- --test-threads=1  # single-threaded
```

The test suite includes 38 integration tests covering:
- File serving (HTML, JS, plain text, binary)
- MIME type detection
- Directory listing (HTML + JSON)
- SPA mode rewrites
- Clean URL redirects
- Range requests (byte ranges, suffix ranges)
- Error pages (HTML + JSON formats)
- Path traversal protection
- serve.json parsing (all configuration keys)
- CLI argument parsing (all flags and combinations)
- Listen URI parsing
- ETag and no-ETag modes
- Query string handling
- URL encoding/decoding

## Current Limitations

- **SSL/TLS**: CLI flags are accepted and parsed, but HTTPS serving is not yet implemented. Use a reverse proxy (nginx, Caddy) for HTTPS.
- **Compression middleware**: The `--no-compression` flag is parsed but gzip middleware is not yet wired into the request pipeline.
- **UNIX domain sockets / Windows named pipes**: Listen endpoint formats are documented but not yet implemented.
- **Trailing slash redirects**: The `trailingSlash` config option is parsed but redirects are not yet active.

## License

MIT
