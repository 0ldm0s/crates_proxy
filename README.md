# Crates Proxy

一个高性能的Rust crates缓存代理服务器，支持上游代理、本地缓存和版本管理。

## 功能特性

- **缓存代理**: 拦截cargo请求，返回缓存内容或从上游获取
- **版本隔离**: 支持同一包的不同版本共存
- **TTL管理**: 缓存过期时间控制
- **上游代理**: 支持HTTP/SOCKS5代理链
- **UA模拟**: 模拟正常浏览器User-Agent

## 安装

```bash
git clone <repository-url>
cd crates_proxy
cargo build --release
```

## 使用方法

### 基本启动

```bash
# 使用默认配置
cargo run

# 指定配置文件
cargo run -- -f config.toml
```

### 管理命令

```bash
# 显示缓存统计
cargo run -- --stats

# 清理过期缓存
cargo run -- --clean

# 显示帮助信息
cargo run -- --help
```

## 配置文件

配置文件使用TOML格式，示例配置：

```toml
[server]
bind_addr = "127.0.0.1:8080"

[cache]
storage_path = "./cache"
default_ttl = 3600  # 秒

[upstream]
# 可选：上游代理配置
# proxy_url = "http://proxy.example.com:8080"
# 或
# proxy_url = "socks5://proxy.example.com:1080"

[user_agent]
value = "Mozilla/5.0 ( compatible crates-proxy/0.1.0 )"

[logging]
level = "info"
```

## 配置cargo使用代理

在 `~/.cargo/config.toml` 中配置：

```toml
[net]
git-fetch-with-cli = true

[source.crates-io]
replace-with = "local-proxy"

[source.local-proxy]
registry = "http://127.0.0.1:8080"
```

## 缓存策略

- 缓存键：`包名-版本号` (如 `serde-1.0.100`)
- 目录结构：`缓存根目录/包名/版本号/文件`
- TTL默认：3600秒（1小时）

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
└── cache/             # 缓存目录
    ├── serde/
    │   ├── 1.0.100/
    │   └── 2.0.0/
    └── bincode/
        ├── 1.3.3/
        └── 2.0.0/
```

## 技术架构

- **HTTP框架**: Hyper - 高性能异步HTTP库
- **HTTP客户端**: libcurl - 支持上游HTTP/SOCKS5代理和二进制文件下载
- **配置格式**: TOML - 简单易读的配置文件格式
- **缓存存储**: 本地文件系统 - 按包名和版本号组织目录结构

## 许可证

LGPLv3 License