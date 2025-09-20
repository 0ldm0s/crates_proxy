# Crates Proxy - Rust包缓存代理服务器

一个高性能的Rust crates.io缓存代理服务器，专门设计用于加速cargo编译过程中的依赖包下载。

## 🚀 特性

- **📦 智能缓存**: 自动缓存所有版本的Rust包，支持复杂的依赖关系
- **⚡ 高性能**: 基于Hyper HTTP服务器和MelangeDB嵌入式数据库
- **🔄 版本管理**: 完整的版本信息管理，支持TTL过期机制
- **🌐 代理支持**: 支持HTTP/SOCKS5代理配置
- **📊 统计监控**: 提供详细的缓存统计和清理功能
- **🛡️ 生产就绪**: 完善的错误处理和日志记录

## 🎯 适用场景

- 企业内网环境下的cargo包加速
- CI/CD流水线中的依赖缓存
- 离线开发环境的包管理
- 多团队共享的包缓存服务

## 📋 系统要求

- Rust 1.70+
- 可用磁盘空间（用于缓存）
- 网络连接（用于首次下载）

## 🛠️ 安装

### 从源码构建

```bash
git clone https://github.com/your-username/crates_proxy.git
cd crates_proxy
cargo build --release
```

### 使用cargo安装

```bash
cargo install crates_proxy
```

## ⚙️ 配置

服务器使用 `config.toml` 文件进行配置。首次运行时会自动创建默认配置文件：

```toml
[server]
bind_addr = "127.0.0.1:8080"

[cache]
storage_path = "./cache"
default_ttl = 3600

[logging]
level = "info"

[user_agent]
value = "Mozilla/5.0 ( compatible crates-proxy/0.1.0 )"

# 可选：代理配置
# [upstream]
# proxy_url = "http://proxy.example.com:8080"
```

## 🚀 运行

### 基本运行

```bash
cargo run
```

### 指定配置文件

```bash
cargo run -- -f /path/to/config.toml
```

### 后台运行

```bash
nohup cargo run > server.log 2>&1 &
```

## 📖 使用方法

### 配置cargo使用代理

在你的项目 `.cargo/config.toml` 中添加：

```toml
[source.crates-io]
replace-with = 'local-registry'

[source.local-registry]
registry = "https://your-proxy-server:8080/index"
```

或者在环境变量中设置：

```bash
export CARGO_HTTP_PROXY=http://127.0.0.1:8080
```

### 直接下载包

```bash
# 下载最新版本
curl http://127.0.0.1:8080/api/v1/crates/tokio/latest/download -o tokio.crate

# 下载指定版本
curl http://127.0.0.1:8080/api/v1/crates/tokio/1.0.0/download -o tokio-1.0.0.crate
```

## 🔧 命令行选项

```bash
crates-proxy [OPTIONS]

选项:
  -f, --config <FILE>     配置文件路径
  -c, --clean             清理过期缓存
  -s, --stats             显示缓存统计信息
  -h, --help              显示帮助信息
  -V, --version           显示版本信息
```

### 示例

```bash
# 清理过期缓存
cargo run -- --clean

# 查看缓存统计
cargo run -- --stats

# 使用自定义配置
cargo run -- -f /path/to/custom_config.toml
```

## 📊 缓存管理

### 查看统计信息

```bash
cargo run -- --stats
```

输出示例：
```
缓存统计信息:
  总文件数: 87
  有效文件数: 87
  过期文件数: 0
  总大小: 605481 字节
```

### 清理过期缓存

```bash
cargo run -- --clean
```

这将清理：
- 过期的文件缓存
- 过期的版本管理数据
- 空的目录结构

## 🏗️ 架构设计

### 核心组件

1. **ProxyService**: 主要的HTTP服务组件
2. **CratesApiClient**: crates.io API客户端
3. **CacheManager**: 文件缓存管理器
4. **VersionManager**: 版本信息管理器（基于MelangeDB）
5. **CurlClient**: HTTP下载客户端

### 缓存策略

- **文件缓存**: 下载的crate文件存储在文件系统
- **版本缓存**: 版本信息存储在MelangeDB中
- **内存缓存**: 热点数据在内存中缓存
- **TTL机制**: 自动过期清理

### 工作流程

1. 接收cargo下载请求
2. 解析包名和版本信息
3. 检查版本管理器获取版本信息
4. 检查文件缓存是否存在
5. 缓存命中则直接返回
6. 缓存未命中则从crates.io下载
7. 保存到缓存并返回给客户端

## 🧪 开发

### 运行测试

```bash
cargo test
```

### 运行示例

```bash
cargo run --example simple_curl_test
```

### 代码结构

```
src/
├── main.rs              # 应用入口
├── proxy.rs             # 代理服务核心
├── crates_api.rs        # crates.io API客户端
├── cache.rs             # 文件缓存管理
├── version_manager.rs   # 版本信息管理
├── curl_client.rs       # HTTP下载客户端
└── config.rs            # 配置管理
```

## 📈 性能优化

- **MelangeDB**: 高性能嵌入式数据库，支持压缩和缓存
- **批量处理**: 一次获取所有版本信息
- **智能缓存**: 多级缓存策略
- **异步处理**: 基于Tokio的异步架构
- **定期清理**: 自动过期数据清理

## 🤝 贡献

欢迎提交Issue和Pull Request！

### 开发流程

1. Fork项目
2. 创建功能分支
3. 提交更改
4. 创建Pull Request

## 📄 许可证

本项目采用LGPLv3许可证。详见[LICENSE](LICENSE)文件。

## 🔗 相关链接

- [crates.io](https://crates.io/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)
- [MelangeDB](https://crates.io/crates/melange_db)
- [Hyper](https://hyper.rs/)

## 📞 支持

如有问题，请提交Issue或联系维护者。

---

**注意**: 本项目仅用于合法的缓存加速用途，请遵守crates.io的使用条款和服务协议。