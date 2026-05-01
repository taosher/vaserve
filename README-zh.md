# vaserve

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**vaserve 是 [vercel/serve](https://github.com/vercel/serve) 的 Rust 版本。**
静态文件服务与目录列表 — CLI 参数和配置与原版 vercel/serve 完全兼容。

## 用户故事

**作为开发者**，在工作中经常需要预览静态网站、单页应用或任何包含静态资源的项目，**我希望**有一个零配置、即时启动的静态文件服务器，**以便**快速预览工作成果、在局域网内分享文件、快速迭代，而无需安装或配置 Node.js、nginx、Apache 等重型 Web 服务器。

### serve 解决了什么问题？

1. **"我只想看看构建产物的效果"**
   你刚跑完 `npm run build`，生成了 `dist/` 文件夹。直接双击打开 `index.html` 文件会有问题——相对路径挂掉、`/api` 请求无响应、前端路由依赖 HTTP 服务器环境。`vaserve dist/` 一条命令就能给你一个功能完备的服务器。

2. **"我需要测试 SPA 的前端路由"**
   你的 React/Vue/Svelte 应用使用了客户端路由（`/about`、`/dashboard`）。直接用文件协议访问只能看到 `index.html` 一个页面。使用 `vaserve -s dist/`，所有未找到的路由都会 rewrite 到 `index.html`——和生产环境完全一致。

3. **"我想在局域网里分享文件"**
   你有一些截图、文档或构建产物要分享给同事。`vaserve` 秒级启动，自动把 URL 复制到剪贴板，加上 `-C` 开启 CORS 后同事可以直接访问——零配置。

4. **"我不想为了一个静态服务器装 Node.js"**
   原版 `serve` 需要 Node.js、npm 以及几百个传递依赖。你工作在 Rust 生态中——或者你只是想要一个单独的、小巧的二进制文件。`vaserve` 是即插即用的替代品：相同的 CLI、相同的 `serve.json` 配置、相同的行为。用 `vaserve` 替代 `npx serve`。

5. **"我需要在 CI/CD 或 Docker 里运行静态服务"**
   一个静态链接的独立二进制文件，无运行时依赖，极小体积。`vaserve dist/` 可以在任何 Rust 能编译的平台上运行——Linux、macOS、Windows、ARM。可以放到 `FROM scratch` 的 Docker 镜像里、CI 流水线中，或者在树莓派上运行。

## 安装

### 从 crates.io 安装

```bash
cargo install vaserve
```

### 从源码编译

```bash
git clone https://github.com/taosher/vaserve.git
cd vaserve
cargo build --release
```

编译产物在 `target/release/vaserve`，拷贝到 `$PATH` 目录即可：

```bash
cp target/release/vaserve /usr/local/bin/
```

### 环境要求

- Rust 1.70+ (MSRV)

## 快速开始

```bash
# 在当前目录启动，默认监听 3000 端口
vaserve

# 指定文件夹
vaserve build/

# 自定义端口
vaserve -l 8080

# SPA 模式（所有路由 rewrite 到 index.html）
vaserve -s dist/

# 开启 CORS
vaserve -C

# 多地址监听
vaserve -l 3000 -l 3001
```

## 使用说明

```
$ vaserve --help
$ vaserve --version
$ vaserve folder_name
$ vaserve [-l listen_uri [-l ...]] [directory]
```

默认情况下，vaserve 监听 `0.0.0.0:3000` 并提供当前工作目录的服务。

## CLI 选项

| 选项 | 别名 | 描述 | 默认值 |
|--------|-------|-------------|---------|
| `--help` | `-h` | 显示帮助信息 | — |
| `--version` | `-v` | 显示版本号 | — |
| `--listen <uri>` | `-l` | 监听 URI（可重复指定） | `0.0.0.0:3000` |
| `-p <port>` |  | 自定义端口（已弃用，请使用 `-l`） | — |
| `--single` | `-s` | SPA 模式：404 请求 rewrite 到 `index.html` | `false` |
| `--debug` | `-d` | 显示调试信息 | `false` |
| `--config <path>` | `-c` | 指定 `serve.json` 路径 | `serve.json` |
| `--no-request-logging` | `-L` | 禁用请求日志 | `false` |
| `--cors` | `-C` | 启用 CORS（`Access-Control-Allow-Origin: *`） | `false` |
| `--no-clipboard` | `-n` | 不复制地址到剪贴板 | `false` |
| `--no-compression` | `-u` | 禁用 gzip 压缩 | `false` |
| `--no-etag` |  | 发送 `Last-Modified` 而非 `ETag` | `false` |
| `--symlinks` | `-S` | 跟随符号链接而非返回 404 | `false` |
| `--ssl-cert <path>` |  | SSL 证书路径（PEM/PKCS12） | — |
| `--ssl-key <path>` |  | SSL 私钥路径 | — |
| `--ssl-pass <path>` |  | SSL 密码短语文件路径 | — |
| `--no-port-switching` |  | 端口被占用时不自动切换 | `false` |

### 监听地址

```bash
# 仅端口（默认绑定 0.0.0.0）
vaserve -l 1234

# 指定主机的 TCP
vaserve -l tcp://hostname:1234

# 主机:端口
vaserve -l 127.0.0.1:3000

# 多地址
vaserve -l tcp://0.0.0.0:3000 -l tcp://0.0.0.0:3001
```

## serve.json 配置

在公共目录中创建 `serve.json` 文件以声明式地配置行为：

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

### 配置参考

| 键 | 类型 | 描述 |
|-----|------|-------------|
| `public` | `string` | 要提供的目录路径 |
| `cleanUrls` | `boolean \| string[]` | 从 URL 中去除 `.html`/`/index` |
| `trailingSlash` | `boolean` | 强制添加/移除尾部斜杠 |
| `rewrites` | `{source, destination}[]` | URL rewrite 规则 |
| `redirects` | `{source, destination, type}[]` | HTTP 重定向规则（默认 301） |
| `headers` | `{source, headers[]}[]` | 按路由设置自定义 HTTP 头 |
| `directoryListing` | `boolean \| string[]` | 启用/禁用目录列表 |
| `unlisted` | `string[]` | 目录列表中隐藏的路径 |
| `renderSingle` | `boolean` | SPA 模式（404 rewrite 到 `index.html`） |
| `symlinks` | `boolean` | 跟踪符号链接 |
| `etag` | `boolean` | 为缓存生成 ETag 头 |

### Rewrite 模式语法

| 模式 | 示例 | 描述 |
|---------|---------|-------------|
| `**` | `**` | 匹配所有路径 |
| `/prefix/**` | `/api/**` | 匹配前缀下的路径 |
| `*` 通配符 | `*.php` | 匹配单个路径段 |
| `:param` | `/user/:id` | 命名路径参数 |

## 功能特性

### 文件服务

- 通过文件扩展名检测提供正确的 `Content-Type` 头
- 所有文件设置 `Content-Disposition: inline`
- `Accept-Ranges: bytes` 支持部分内容请求
- 自动 gzip 压缩（可通过 `--no-compression` 禁用）

### ETag 缓存

基于 SHA-1 的 ETag，按文件缓存并通过 mtime 失效。使用 `--no-etag` 可切换为 `Last-Modified`。

### Range 请求

完整支持 HTTP 字节范围请求：

```bash
curl -H "Range: bytes=0-1023" http://localhost:3000/large-file.bin
curl -H "Range: bytes=-500" http://localhost:3000/large-file.bin
```

无效的范围返回 `416 Range Not Satisfiable`。

### 目录列表

简洁、响应式的 HTML 目录列表，与 serve 原版设计一致：
- 目录优先排列，各自内部按字母排序
- 面包屑导航
- SVG CSS 背景的文件夹和文件图标
- 设置 `Accept: application/json` 时输出 JSON 格式
- 过滤隐藏文件（`.DS_Store`、`.git`、自定义 `unlisted` 模式）

### 错误页面

- **400 Bad Request** — 路径穿越或格式错误的 URL
- **404 Not Found** — 文件缺失、符号链接已禁用
- **500 Internal Server Error** — 未处理的异常
- HTML 输出（匹配 serve 设计风格）或 JSON（`Accept: application/json`）
- 支持自定义错误页面：在公共目录中放置 `404.html`

### 端口自动切换

当请求的端口被占用时，serve 自动选择一个可用端口。使用 `--no-port-switching` 禁用。

### 剪贴板

启动时自动将本地 URL 复制到剪贴板。使用 `--no-clipboard` 禁用。

### SPA 模式

`--single` / `-s` 启用单页应用模式。所有未找到的请求都 rewrite 到 `index.html`，使客户端路由正常工作。

## 架构

```
src/
├── main.rs       # 入口 — CLI 和服务器编排
├── lib.rs        # 库根（暴露模块供测试使用）
├── cli.rs        # CLI 参数解析（clap derive）
├── config.rs     # serve.json 反序列化 + CLI 合并
├── server.rs     # HTTP 服务器、端口切换、启动消息
├── handler.rs    # 核心请求处理器（serve-handler 逻辑）
└── templates.rs  # 目录列表和错误页面的 HTML 模板
```

### 技术栈

| 组件 | Crate |
|-----------|-------|
| HTTP 服务器 | `axum` 0.7 + `hyper` 1 |
| 异步运行时 | `tokio` |
| CLI 解析 | `clap` 4 (derive) |
| JSON 配置 | `serde` + `serde_json` |
| MIME 类型 | `mime_guess` |
| ETag | `sha1` |
| 剪贴板 | `arboard` |
| Gzip | `flate2` |

## 开发指南

### 构建

```bash
cargo build
cargo build --release
```

### 运行

```bash
cargo run -- -l 3000 ./public
```

### 测试

```bash
cargo test
cargo test -- --test-threads=1  # 单线程执行
```

测试套件包含 38 个集成测试，覆盖：
- 文件服务（HTML、JS、纯文本、二进制）
- MIME 类型检测
- 目录列表（HTML + JSON）
- SPA 模式 rewrite
- Clean URL 重定向
- Range 请求（字节范围、后缀范围）
- 错误页面（HTML + JSON 格式）
- 路径穿越保护
- serve.json 解析（所有配置键）
- CLI 参数解析（所有标志和组合）
- 监听 URI 解析
- ETag 和 no-ETag 模式
- 查询字符串处理
- URL 编码/解码

## 当前局限

- **SSL/TLS**：CLI 参数已接受和解析，但 HTTPS 服务尚未实现。请使用反向代理（nginx、Caddy）提供 HTTPS。
- **压缩中间件**：`--no-compression` 参数已解析，但 gzip 中间件尚未接入请求管道。
- **UNIX 域套接字 / Windows 命名管道**：监听地址格式已文档化但尚未实现。
- **尾部斜杠重定向**：`trailingSlash` 配置选项已解析但重定向尚未激活。

## 许可证

MIT
