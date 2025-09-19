# Crates Proxy 设计文档

## 概述

一个Rust crates缓存代理服务器，支持上游代理、本地缓存和版本管理。

## 技术选型

- **HTTP框架**: Hyper - 高性能异步HTTP库
- **HTTP客户端**: libcurl (通过curl或curl-rs) - 支持上游HTTP/SOCKS5代理和二进制文件下载
- **配置格式**: TOML - 简单易读的配置文件格式
- **缓存存储**: 本地文件系统 - 按包名和版本号组织目录结构

## 功能特性

### 核心功能
1. 缓存代理：拦截cargo请求，返回缓存内容或从上游获取
2. 版本隔离：支持同一包的不同版本共存
3. TTL管理：缓存过期时间控制
4. 上游代理：支持HTTP/SOCKS5代理链
5. UA模拟：模拟正常浏览器User-Agent

### 缓存策略
- 缓存键：`包名-版本号` (如 `serde-1.0.100`)
- 目录结构：`缓存根目录/包名/版本号/文件`
- TTL默认：开发环境5分钟，生产环境2小时

## 配置文件结构

```toml
[server]
bind_addr = "127.0.0.1:8080"

[cache]
storage_path = "./cache"
default_ttl = 3600  # 秒

[upstream]
# 可选：上游代理配置
proxy_url = "http://proxy.example.com:8080"
# 或
# proxy_url = "socks5://proxy.example.com:1080"

[user_agent]
# 模拟User-Agent
value = "Mozilla/5.0 ( compatible crates-proxy/0.1.0 )"

[logging]
level = "info"
```

## 工作流程

### 场景1：缓存命中
1. cargo请求 → crates_proxy接收
2. 检查缓存是否存在且TTL有效
3. 命中则直接返回缓存内容
4. 记录命中统计

### 场景2：缓存未命中或过期
1. cargo请求 → crates_proxy接收
2. 缓存不存在或TTL过期
3. 通过libcurl向上游请求（支持代理链）
4. 版本检查：
   - 版本一致：刷新TTL，返回缓存
   - 版本不一致：下载最新内容，更新缓存，返回新内容

## 版本兼容性

- 完全支持多版本共存
- 每个版本独立缓存路径
- 版本检查基于特定版本进行
- 兼容cargo的版本解析机制

## 目录结构

```
crates_proxy/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── config.rs      # 配置解析
│   ├── cache.rs       # 缓存管理
│   ├── proxy.rs       # 代理逻辑
│   └── curl_client.rs # libcurl客户端
├── config.toml        # 配置文件
└── cache/             # 缓存目录（可配置）
    ├── serde/
    │   ├── 1.0.100/
    │   └── 2.0.0/
    └── bincode/
        ├── 1.3.3/
        └── 2.0.0/
```

## 依赖项

```toml
[dependencies]
hyper = { version = "1.0", features = ["full"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
curl = "0.4"  # 或 curl-rs
anyhow = "1.0"
thiserror = "1.0"
```

## 启动方式

```bash
# 使用默认配置
cargo run

# 指定配置文件
cargo run -- --config config.toml
```

## 配置cargo使用代理

在`~/.cargo/config.toml`中配置：
```toml
[net]
git-fetch-with-cli = true

[source.crates-io]
replace-with = "local-proxy"

[source.local-proxy]
registry = "http://127.0.0.1:8080"
```