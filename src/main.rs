mod cache;
mod config;
mod crates_api;
mod curl_client;
mod proxy;
mod version_manager;

use clap::Parser;
use config::{Config, ConfigError};
use proxy::run_server;
use rat_logger::{self, LevelFilter, FileConfig, FormatConfig};
use rat_logger::producer_consumer::BatchConfig;
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

    // 根据日志级别决定是否启用开发模式
    let dev_mode = matches!(log_level, LevelFilter::Debug | LevelFilter::Trace);

    // 配置文件输出（始终使用简洁格式）
    let file_format = FormatConfig {
        timestamp_format: "%Y-%m-%d %H:%M:%S%.3f".to_string(),
        level_style: rat_logger::LevelStyle {
            error: "ERROR".to_string(),
            warn: "WARN ".to_string(),
            info: "INFO ".to_string(),
            debug: "DEBUG".to_string(),
            trace: "TRACE".to_string(),
        },
        format_template: "{timestamp} [{level}] {message}".to_string(),
    };

    let file_config = FileConfig {
        log_dir: PathBuf::from("./logs"),
        max_file_size: 10 * 1024 * 1024, // 10MB
        max_compressed_files: 5,
        compression_level: 6,
        min_compress_threads: 1,
        skip_server_logs: false,
        is_raw: false,
        compress_on_drop: true,
        format: Some(file_format),
    };

    // 小负载服务器的文件日志配置：在性能和可靠性之间取得平衡
    let mut builder = rat_logger::LoggerBuilder::new()
        .add_file(file_config)
        .with_batch_config(BatchConfig {
            batch_size: 512,        // 512字节批量大小，适中的批量处理
            batch_interval_ms: 10,  // 10ms刷新间隔，确保及时写入
            buffer_size: 1024,      // 1KB缓冲区，足够的缓冲空间
        })
        .with_level(log_level);

    // 根据是否为开发模式配置终端输出格式
    if dev_mode {
        // 开发模式：保留详细格式便于调试
        let dev_format = FormatConfig {
            timestamp_format: "%Y-%m-%d %H:%M:%S%.3f".to_string(),
            level_style: rat_logger::LevelStyle {
                error: "ERROR".to_string(),
                warn: "WARN ".to_string(),
                info: "INFO ".to_string(),
                debug: "DEBUG".to_string(),
                trace: "TRACE".to_string(),
            },
            format_template: "{timestamp} [{level}] {target}:{line} - {message}".to_string(),
        };

        builder = builder
            .add_terminal_with_config(rat_logger::handler::term::TermConfig {
                enable_color: true,
                enable_async: false, // 开发模式禁用异步确保立即输出
                batch_size: 1,
                flush_interval_ms: 1,
                format: Some(dev_format),
                color: None, // 使用默认颜色
            })
            .with_dev_mode(true); // 确保日志立即输出
    } else {
        // 生产模式：简洁格式，只显示时间、级别和消息
        let prod_format = FormatConfig {
            timestamp_format: "%Y-%m-%d %H:%M:%S%.3f".to_string(),
            level_style: rat_logger::LevelStyle {
                error: "ERROR".to_string(),
                warn: "WARN ".to_string(),
                info: "INFO ".to_string(),
                debug: "DEBUG".to_string(),
                trace: "TRACE".to_string(),
            },
            format_template: "{timestamp} [{level}] {message}".to_string(),
        };

        builder = builder
            .add_terminal_with_config(rat_logger::handler::term::TermConfig {
                enable_color: true,
                enable_async: false, // 改为同步输出确保日志立即显示
                batch_size: 1,
                flush_interval_ms: 1,
                format: Some(prod_format),
                color: None, // 使用默认颜色
            });
    }

    if let Err(e) = builder.init() {
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

/// 清理melange_db锁文件
fn cleanup_melange_db_locks(config: &Config) {
    use std::path::Path;

    // 版本管理器数据库路径
    let versions_db_path = Path::new(&config.cache.storage_path).join("versions_db");

    // 清理版本管理器数据库锁文件
    if let Err(e) = melange_db::cleanup_lock_files(&versions_db_path) {
        rat_logger::warn!("清理版本管理器锁文件失败: {}", e);
    } else {
        rat_logger::info!("已检查版本管理器锁文件");
    }

    // 检查是否还有其他可能的melange_db实例
    // 这里可以添加更多数据库路径的检查
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

    // 清理melange_db锁文件
    cleanup_melange_db_locks(&config);

    // 处理清理缓存命令
    if args.clean {
        println!("正在清理过期缓存...");

        // 清理文件缓存
        match cache::CacheManager::new(&config.cache.storage_path, config.cache.default_ttl) {
            Ok(cache_manager) => {
                if let Err(e) = cache_manager.clear_expired_cache() {
                    eprintln!("清理文件缓存失败: {}", e);
                    process::exit(1);
                }
                println!("文件缓存清理完成");
            }
            Err(e) => {
                eprintln!("创建缓存管理器失败: {}", e);
                process::exit(1);
            }
        }

        // 清理版本管理器数据
        match version_manager::VersionManager::new(&config) {
            Ok(version_manager) => {
                match version_manager.cleanup_expired_data() {
                    Ok(count) => println!("版本管理器清理完成，清理了 {} 个过期数据", count),
                    Err(e) => {
                        eprintln!("清理版本管理器数据失败: {}", e);
                        process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("创建版本管理器失败: {}", e);
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
