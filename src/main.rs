mod cache;
mod config;
mod crates_api;
mod curl_client;
mod proxy;

use clap::Parser;
use config::{Config, ConfigError};
use proxy::run_server;
use rat_logger::{self, LevelFilter, FileConfig};
use std::process;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "crates-proxy")]
#[command(about = "Rust crates缓存代理服务器")]
#[command(version = "0.1.0")]
struct Args {
    #[arg(short = 'f', long, help = "配置文件路径")]
    config: Option<String>,

    #[arg(short, long, help = "清理过期缓存")]
    clean: bool,

    #[arg(short, long, help = "显示缓存统计")]
    stats: bool,
}

fn setup_logging(level: &str) {
    // 转换日志级别
    let log_level = match level {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };

    // 初始化rat_logger，添加终端和文件输出
    let file_config = FileConfig {
        log_dir: PathBuf::from("./logs"),
        max_file_size: 10 * 1024 * 1024, // 10MB
        max_compressed_files: 5,
        compression_level: 6,
        min_compress_threads: 1,
        skip_server_logs: false,
        is_raw: false,
        compress_on_drop: true,
    };

    if let Err(e) = rat_logger::LoggerBuilder::new()
        .add_terminal()
        .add_file(file_config)
        .with_level(log_level)
        .init()
    {
        eprintln!("日志初始化失败: {}", e);
        process::exit(1);
    }
}

fn load_config(config_path: Option<String>) -> Result<Config, ConfigError> {
    match config_path {
        Some(path) => Config::from_file(path),
        None => Ok(Config::default()),
    }
}

fn main() {
    let args = Args::parse();

    // 加载配置
    let config = match load_config(args.config) {
        Ok(config) => {
            if let Err(e) = config.validate() {
                eprintln!("配置验证失败: {}", e);
                process::exit(1);
            }
            config
        }
        Err(e) => {
            eprintln!("加载配置失败: {}", e);
            process::exit(1);
        }
    };

    // 设置日志
    setup_logging(&config.logging.level);

    // 处理清理缓存命令
    if args.clean {
        println!("正在清理过期缓存...");
        match cache::CacheManager::new(&config.cache.storage_path, config.cache.default_ttl) {
            Ok(cache_manager) => {
                if let Err(e) = cache_manager.clear_expired_cache() {
                    eprintln!("清理缓存失败: {}", e);
                    process::exit(1);
                }
                println!("缓存清理完成");
            }
            Err(e) => {
                eprintln!("创建缓存管理器失败: {}", e);
                process::exit(1);
            }
        }
        return;
    }

    // 处理显示统计信息
    if args.stats {
        println!("缓存统计信息:");
        match cache::CacheManager::new(&config.cache.storage_path, config.cache.default_ttl) {
            Ok(cache_manager) => {
                match cache_manager.get_cache_stats() {
                    Ok(stats) => {
                        println!("  总文件数: {}", stats.total_files);
                        println!("  有效文件数: {}", stats.valid_files);
                        println!("  过期文件数: {}", stats.expired_files);
                        println!("  总大小: {} 字节", stats.total_size);
                    }
                    Err(e) => {
                        eprintln!("获取缓存统计失败: {}", e);
                        process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("创建缓存管理器失败: {}", e);
                process::exit(1);
            }
        }
        return;
    }

    // 启动服务器
    println!("启动crates代理服务器...");
    println!("监听地址: {}", config.server.bind_addr);
    println!("缓存路径: {}", config.cache.storage_path);
    println!("默认TTL: {} 秒", config.cache.default_ttl);

    if let Some(upstream) = &config.upstream {
        if let Some(proxy_url) = &upstream.proxy_url {
            println!("上游代理: {}", proxy_url);
        }
    }

    // 设置tokio运行时
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        if let Err(e) = run_server(&config).await {
            eprintln!("服务器运行错误: {}", e);
            process::exit(1);
        }
    });
}
