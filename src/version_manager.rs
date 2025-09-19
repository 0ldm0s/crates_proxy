use crate::config::Config;
use melange_db::{Db, Config as DbConfig, Tree};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// 版本信息数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// 版本号
    pub version: String,
    /// 下载路径
    pub download_path: String,
    /// 校验和
    pub checksum: String,
    /// 是否被撤销
    pub yanked: bool,
    /// 创建时间戳
    pub created_at: u64,
    /// 过期时间戳
    pub expires_at: u64,
}

/// 包的最新版本映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersionMapping {
    /// 包名
    pub crate_name: String,
    /// 最新版本号
    pub latest_version: String,
    /// 最后更新时间
    pub updated_at: u64,
    /// TTL 过期时间
    pub expires_at: u64,
}

/// MelangeDB版本管理器
pub struct VersionManager {
    /// 数据库实例
    db: Arc<Db<1024>>,
    /// 版本信息树
    versions_tree: Arc<Tree<1024>>,
    /// 最新版本映射树
    latest_tree: Arc<Tree<1024>>,
    /// 内存缓存（用于快速访问）
    memory_cache: Arc<RwLock<HashMap<String, String>>>,
    /// 默认TTL
    default_ttl: Duration,
}

#[derive(Debug, Error)]
pub enum VersionManagerError {
    #[error("数据库错误: {0}")]
    DatabaseError(#[from] io::Error),
    #[error("序列化错误: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("系统时间错误: {0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),
    #[error("数据过期: {0}")]
    ExpiredError(String),
    #[error("数据不存在: {0}")]
    NotFoundError(String),
}

impl VersionManager {
    /// 创建新的版本管理器
    pub fn new(config: &Config) -> Result<Self, VersionManagerError> {
        let db_path = Path::new(&config.cache.storage_path).join("versions_db");

        // 创建数据库配置
        let mut db_config = DbConfig::new()
            .path(&db_path)
            .cache_capacity_bytes(100 * 1024 * 1024) // 100MB缓存
            .flush_every_ms(Some(5000)); // 5秒flush间隔

        // 启用智能flush策略
        db_config.smart_flush_config.enabled = true;
        db_config.smart_flush_config.base_interval_ms = 5000;
        db_config.smart_flush_config.min_interval_ms = 1000;
        db_config.smart_flush_config.max_interval_ms = 30000;

        // 创建数据库
        let db = Arc::new(db_config.open()?);

        // 打开数据树
        let versions_tree = Arc::new(db.open_tree(b"versions")?);
        let latest_tree = Arc::new(db.open_tree(b"latest_versions")?);

        rat_logger::info!("版本管理器初始化成功，数据库路径: {:?}", db_path);

        Ok(Self {
            db,
            versions_tree,
            latest_tree,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl: Duration::from_secs(config.cache.default_ttl),
        })
    }

    /// 获取包的最新版本号
    pub fn get_latest_version(&self, crate_name: &str) -> Result<Option<String>, VersionManagerError> {
        // 首先检查内存缓存
        {
            let cache = self.memory_cache.read().unwrap();
            if let Some(version) = cache.get(crate_name) {
                rat_logger::debug!("从内存缓存获取版本: {} -> {}", crate_name, version);
                return Ok(Some(version.clone()));
            }
        }

        // 检查数据库
        let key = crate_name.as_bytes();
        if let Some(data) = self.latest_tree.get(key)? {
            let mapping: LatestVersionMapping = serde_json::from_slice(&data)?;

            // 检查是否过期
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            if current_time > mapping.expires_at {
                rat_logger::warn!("最新版本映射已过期: {} -> {}", crate_name, mapping.latest_version);
                self.latest_tree.remove(key)?;
                return Ok(None);
            }

            // 更新内存缓存
            {
                let mut cache = self.memory_cache.write().unwrap();
                cache.insert(crate_name.to_string(), mapping.latest_version.clone());
            }

            rat_logger::info!("从数据库获取最新版本: {} -> {}", crate_name, mapping.latest_version);
            Ok(Some(mapping.latest_version))
        } else {
            Ok(None)
        }
    }

    /// 设置包的最新版本号
    pub fn set_latest_version(&self, crate_name: &str, version: &str) -> Result<(), VersionManagerError> {
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let expires_at = current_time + self.default_ttl.as_secs();

        let mapping = LatestVersionMapping {
            crate_name: crate_name.to_string(),
            latest_version: version.to_string(),
            updated_at: current_time,
            expires_at,
        };

        let data = serde_json::to_vec(&mapping)?;
        self.latest_tree.insert(crate_name.as_bytes(), data)?;

        // 更新内存缓存
        {
            let mut cache = self.memory_cache.write().unwrap();
            cache.insert(crate_name.to_string(), version.to_string());
        }

        rat_logger::info!("设置最新版本: {} -> {} (TTL: {}s)", crate_name, version, self.default_ttl.as_secs());
        Ok(())
    }

    /// 获取版本信息
    pub fn get_version_info(&self, crate_name: &str, version: &str) -> Result<Option<VersionInfo>, VersionManagerError> {
        let key = format!("{}:{}", crate_name, version);
        if let Some(data) = self.versions_tree.get(key.as_bytes())? {
            let version_info: VersionInfo = serde_json::from_slice(&data)?;

            // 检查是否过期
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            if current_time > version_info.expires_at {
                rat_logger::warn!("版本信息已过期: {}:{} -> {}", crate_name, version, version_info.version);
                self.versions_tree.remove(key.as_bytes())?;
                return Ok(None);
            }

            Ok(Some(version_info))
        } else {
            Ok(None)
        }
    }

    /// 设置版本信息
    pub fn set_version_info(&self, crate_name: &str, version: &str, version_info: VersionInfo) -> Result<(), VersionManagerError> {
        let key = format!("{}:{}", crate_name, version);
        let data = serde_json::to_vec(&version_info)?;
        self.versions_tree.insert(key.as_bytes(), data)?;

        rat_logger::info!("设置版本信息: {}:{} -> {}", crate_name, version, version_info.version);
        Ok(())
    }

    /// 获取包的所有版本
    pub fn get_all_versions(&self, crate_name: &str) -> Result<Vec<VersionInfo>, VersionManagerError> {
        let prefix = format!("{}:", crate_name);
        let mut versions = Vec::new();
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        for kv in self.versions_tree.scan_prefix(prefix.as_bytes()) {
            let (key, value) = kv?;
            if let Ok(version_info) = serde_json::from_slice::<VersionInfo>(&value) {
                if current_time <= version_info.expires_at {
                    versions.push(version_info);
                } else {
                    // 清理过期数据
                    self.versions_tree.remove(&key)?;
                }
            }
        }

        rat_logger::info!("获取包 {} 的所有版本，共 {} 个", crate_name, versions.len());
        Ok(versions)
    }

    /// 创建版本信息
    pub fn create_version_info(
        &self,
        crate_name: &str,
        version: &str,
        download_path: &str,
        checksum: &str,
        yanked: bool,
    ) -> Result<VersionInfo, VersionManagerError> {
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let expires_at = current_time + self.default_ttl.as_secs();

        let version_info = VersionInfo {
            version: version.to_string(),
            download_path: download_path.to_string(),
            checksum: checksum.to_string(),
            yanked,
            created_at: current_time,
            expires_at,
        };

        self.set_version_info(crate_name, version, version_info.clone())?;
        Ok(version_info)
    }

    /// 清理过期数据
    pub fn cleanup_expired_data(&self) -> Result<usize, VersionManagerError> {
        let mut cleaned_count = 0;
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // 清理过期版本信息
        for kv in self.versions_tree.iter() {
            let (key, value) = kv?;
            if let Ok(version_info) = serde_json::from_slice::<VersionInfo>(&value) {
                if current_time > version_info.expires_at {
                    self.versions_tree.remove(&key)?;
                    cleaned_count += 1;
                }
            }
        }

        // 清理过期最新版本映射
        for kv in self.latest_tree.iter() {
            let (key, value) = kv?;
            if let Ok(mapping) = serde_json::from_slice::<LatestVersionMapping>(&value) {
                if current_time > mapping.expires_at {
                    self.latest_tree.remove(&key)?;
                    cleaned_count += 1;

                    // 同时清理内存缓存
                    let mut cache = self.memory_cache.write().unwrap();
                    cache.remove(&mapping.crate_name);
                }
            }
        }

        if cleaned_count > 0 {
            rat_logger::info!("清理了 {} 个过期数据", cleaned_count);
        }

        Ok(cleaned_count)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> Result<VersionManagerStats, VersionManagerError> {
        let mut latest_count = 0;
        let mut version_count = 0;
        let mut expired_count = 0;
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        // 统计最新版本映射
        for kv in self.latest_tree.iter() {
            let (_, value) = kv?;
            if let Ok(mapping) = serde_json::from_slice::<LatestVersionMapping>(&value) {
                latest_count += 1;
                if current_time > mapping.expires_at {
                    expired_count += 1;
                }
            }
        }

        // 统计版本信息
        for kv in self.versions_tree.iter() {
            let (_, value) = kv?;
            if let Ok(version_info) = serde_json::from_slice::<VersionInfo>(&value) {
                version_count += 1;
                if current_time > version_info.expires_at {
                    expired_count += 1;
                }
            }
        }

        let memory_cache_size = self.memory_cache.read().unwrap().len();

        Ok(VersionManagerStats {
            latest_mappings_count: latest_count,
            versions_count: version_count,
            expired_count,
            memory_cache_size,
        })
    }

    /// 强制刷新数据库
    pub fn flush(&self) -> Result<(), VersionManagerError> {
        self.db.flush()?;
        rat_logger::info!("版本管理器数据库已刷新");
        Ok(())
    }
}

/// 版本管理器统计信息
#[derive(Debug, Clone)]
pub struct VersionManagerStats {
    /// 最新版本映射数量
    pub latest_mappings_count: usize,
    /// 版本信息数量
    pub versions_count: usize,
    /// 过期数据数量
    pub expired_count: usize,
    /// 内存缓存大小
    pub memory_cache_size: usize,
}

impl Drop for VersionManager {
    fn drop(&mut self) {
        if let Err(e) = self.flush() {
            rat_logger::error!("版本管理器销毁时刷新失败: {}", e);
        }
    }
}