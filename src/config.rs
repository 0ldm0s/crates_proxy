use serde::Deserialize;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("配置文件读取失败: {0}")]
    IoError(#[from] std::io::Error),
    #[error("配置文件解析失败: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("绑定地址格式错误: {0}")]
    BindAddrError(String),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub cache: CacheConfig,
    pub upstream: Option<UpstreamConfig>,
    pub user_agent: UserAgentConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: String,
}

#[derive(Debug, Deserialize)]
pub struct CacheConfig {
    pub storage_path: String,
    pub default_ttl: u64,
}

#[derive(Debug, Deserialize)]
pub struct UpstreamConfig {
    pub proxy_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UserAgentConfig {
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        // 验证绑定地址格式
        if !self.server.bind_addr.contains(':') {
            return Err(ConfigError::BindAddrError(
                "绑定地址必须包含端口号".to_string(),
            ));
        }

        // 验证缓存目录
        fs::create_dir_all(&self.cache.storage_path)?;

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_addr: "127.0.0.1:8080".to_string(),
            },
            cache: CacheConfig {
                storage_path: "./cache".to_string(),
                default_ttl: 3600,
            },
            upstream: None,
            user_agent: UserAgentConfig {
                value: "Mozilla/5.0 ( compatible crates-proxy/0.1.0 )".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
            },
        }
    }
}