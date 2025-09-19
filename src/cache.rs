use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
    #[error("路径构建错误: {0}")]
    PathError(String),
}

#[derive(Debug)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub created_at: u64,
    pub ttl: u64,
}

#[derive(Debug)]
pub struct CacheManager {
    storage_path: PathBuf,
    default_ttl: u64,
}

impl CacheManager {
    pub fn new<P: AsRef<Path>>(storage_path: P, default_ttl: u64) -> Result<Self, CacheError> {
        let storage_path = storage_path.as_ref().to_path_buf();
        fs::create_dir_all(&storage_path)?;

        Ok(Self {
            storage_path,
            default_ttl,
        })
    }

    pub fn get_cache_path(&self, crate_name: &str, version: &str, filename: &str) -> PathBuf {
        let path = self.storage_path
            .join(crate_name)
            .join(version)
            .join(filename);

        // 确保目录存在
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                rat_logger::error!("创建缓存目录失败: {:?}, 错误: {}", parent, e);
            }
        }

        path
    }

    pub fn is_cached(&self, crate_name: &str, version: &str, filename: &str) -> bool {
        let path = self.get_cache_path(crate_name, version, filename);
        path.exists() // 临时禁用TTL检查
    }

    pub fn is_expired(&self, path: &Path) -> bool {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(created) = metadata.created() {
                if let Ok(duration) = created.duration_since(UNIX_EPOCH) {
                    let elapsed = duration.as_secs();
                    return elapsed > self.default_ttl;
                }
            }
        }
        true
    }

    pub fn get_cached_content(&self, crate_name: &str, version: &str, filename: &str) -> Result<Vec<u8>, CacheError> {
        let path = self.get_cache_path(crate_name, version, filename);

        if !self.is_cached(crate_name, version, filename) {
            return Err(CacheError::PathError("缓存不存在或已过期".to_string()));
        }

        Ok(fs::read(path)?)
    }

    pub fn save_to_cache(&self, crate_name: &str, version: &str, filename: &str, content: &[u8]) -> Result<(), CacheError> {
        let path = self.get_cache_path(crate_name, version, filename);

        // 创建目录结构
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(())
    }

    pub fn clear_expired_cache(&self) -> Result<(), CacheError> {
        self.clear_expired_cache_recursive(&self.storage_path)?;
        Ok(())
    }

    fn clear_expired_cache_recursive(&self, dir: &Path) -> Result<(), CacheError> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.clear_expired_cache_recursive(&path)?;

                // 如果目录为空，删除它
                if fs::read_dir(&path)?.next().is_none() {
                    fs::remove_dir(&path)?;
                }
            } else {
                if self.is_expired(&path) {
                    fs::remove_file(&path)?;
                }
            }
        }

        Ok(())
    }

    pub fn get_cache_stats(&self) -> Result<CacheStats, CacheError> {
        let mut stats = CacheStats::default();
        self.calculate_stats_recursive(&self.storage_path, &mut stats)?;
        Ok(stats)
    }

    fn calculate_stats_recursive(&self, dir: &Path, stats: &mut CacheStats) -> Result<(), CacheError> {
        if !dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.calculate_stats_recursive(&path, stats)?;
            } else {
                stats.total_files += 1;
                if let Ok(metadata) = fs::metadata(&path) {
                    stats.total_size += metadata.len();
                }

                if self.is_expired(&path) {
                    stats.expired_files += 1;
                } else {
                    stats.valid_files += 1;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub total_files: u64,
    pub valid_files: u64,
    pub expired_files: u64,
    pub total_size: u64,
}