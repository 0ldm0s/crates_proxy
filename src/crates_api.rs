use crate::config::Config;
use curl::easy::{Easy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CrateVersion {
    pub num: String,
    pub dl_path: String,
    pub checksum: String,
    pub yanked: bool,
}

#[derive(Debug, Clone)]
pub struct CrateInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub max_version: String,
    pub downloads: u64,
    pub versions: Vec<u64>, // 版本ID列表
}

#[derive(Debug)]
pub struct CratesApiClient {
    proxy_url: Option<String>,
    user_agent: String,
    timeout: Duration,
}

impl CratesApiClient {
    pub fn new(config: &Config) -> Self {
        let proxy_url = config.upstream
            .as_ref()
            .and_then(|upstream| upstream.proxy_url.clone());

        let user_agent = config.user_agent.value.clone();

        Self {
            proxy_url,
            user_agent,
            timeout: Duration::from_secs(30),
        }
    }

    /// 获取包的基本信息
    pub fn get_crate_info(&self, crate_name: &str) -> Result<CrateInfo, ApiError> {
        let api_url = format!("https://crates.io/api/v1/crates/{}", crate_name);

        let mut handle = Easy::new();
        handle.url(&api_url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;
        handle.follow_location(true)?;
        handle.verbose(false)?;

        // 设置代理
        if let Some(ref proxy_url) = self.proxy_url {
            handle.proxy(proxy_url)?;
        }

        let mut data = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer.write_function(|buf| {
                data.extend_from_slice(buf);
                Ok(buf.len())
            })?;
            transfer.perform()?;
        }

        let response_code = handle.response_code()?;
        if response_code != 200 {
            return Err(ApiError::HttpError(response_code, String::from_utf8_lossy(&data).to_string()));
        }

        let response_text = String::from_utf8(data)?;
        let json: Value = serde_json::from_str(&response_text)?;

        let crate_info = json.get("crate")
            .ok_or_else(|| ApiError::ParseError("缺少 'crate' 字段".to_string()))?;

        let id = crate_info.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::ParseError("缺少 'id' 字段".to_string()))?
            .to_string();

        let name = crate_info.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::ParseError("缺少 'name' 字段".to_string()))?
            .to_string();

        let description = crate_info.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let max_version = crate_info.get("max_version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::ParseError("缺少 'max_version' 字段".to_string()))?
            .to_string();

        let downloads = crate_info.get("downloads")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let versions = json.get("versions")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_u64())
                .collect())
            .unwrap_or_default();

        Ok(CrateInfo {
            id,
            name,
            description,
            max_version,
            downloads,
            versions,
        })
    }

    /// 下载指定版本的包文件
    pub fn download_crate_version(
        &self,
        crate_name: &str,
        version: &str,
        save_path: &Path,
    ) -> Result<(), ApiError> {
        let download_url = format!("https://crates.io/api/v1/crates/{}/{}/download", crate_name, version);

        let mut handle = Easy::new();
        handle.url(&download_url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;
        handle.follow_location(true)?;
        handle.verbose(false)?;

        // 设置代理
        if let Some(ref proxy_url) = self.proxy_url {
            handle.proxy(proxy_url)?;
        }

        let mut data = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer.write_function(|buf| {
                data.extend_from_slice(buf);
                Ok(buf.len())
            })?;
            transfer.perform()?;
        }

        let response_code = handle.response_code()?;
        if response_code != 200 {
            return Err(ApiError::DownloadFailed(response_code, format!("下载失败: HTTP {}", response_code)));
        }

        // 验证文件格式
        if !data.starts_with(&[0x1f, 0x8b]) {
            return Err(ApiError::InvalidFileFormat("文件不是有效的gzip格式".to_string()));
        }

        // 保存文件
        std::fs::write(save_path, &data)
            .map_err(|e| ApiError::IoError(format!("保存文件失败: {}", e)))?;

        Ok(())
    }

    /// 获取包的版本信息
    pub fn get_available_versions(&self, crate_name: &str) -> Result<Vec<CrateVersion>, ApiError> {
        let api_url = format!("https://crates.io/api/v1/crates/{}", crate_name);

        let mut handle = Easy::new();
        handle.url(&api_url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;
        handle.follow_location(true)?;
        handle.verbose(false)?;

        // 设置代理
        if let Some(ref proxy_url) = self.proxy_url {
            handle.proxy(proxy_url)?;
        }

        let mut data = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer.write_function(|buf| {
                data.extend_from_slice(buf);
                Ok(buf.len())
            })?;
            transfer.perform()?;
        }

        let response_code = handle.response_code()?;
        if response_code != 200 {
            return Err(ApiError::HttpError(response_code, String::from_utf8_lossy(&data).to_string()));
        }

        let response_text = String::from_utf8(data)?;
        let json: Value = serde_json::from_str(&response_text)?;

        let mut versions = Vec::new();
        if let Some(version_ids) = json.get("versions").and_then(|v| v.as_array()) {
            for version_id in version_ids {
                if let Some(id_num) = version_id.as_u64() {
                    // 构造版本详情API的URL
                    let version_url = format!("https://crates.io/api/v1/crates/{}/versions/{}", crate_name, id_num);

                    // 获取版本详情
                    if let Ok(version_info) = self.get_version_details(&version_url) {
                        versions.push(version_info);
                    }
                }
            }
        }

        Ok(versions)
    }

    /// 获取特定版本的详细信息
    fn get_version_details(&self, version_url: &str) -> Result<CrateVersion, ApiError> {
        let mut handle = Easy::new();
        handle.url(version_url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;
        handle.follow_location(true)?;
        handle.verbose(false)?;

        // 设置代理
        if let Some(ref proxy_url) = self.proxy_url {
            handle.proxy(proxy_url)?;
        }

        let mut data = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer.write_function(|buf| {
                data.extend_from_slice(buf);
                Ok(buf.len())
            })?;
            transfer.perform()?;
        }

        let response_code = handle.response_code()?;
        if response_code != 200 {
            return Err(ApiError::HttpError(response_code, format!("获取版本详情失败: {}", version_url)));
        }

        let response_text = String::from_utf8(data)?;
        let json: Value = serde_json::from_str(&response_text)?;

        let version = json.get("version")
            .ok_or_else(|| ApiError::ParseError("缺少 'version' 字段".to_string()))?;

        let num = version.get("num")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApiError::ParseError("缺少 'num' 字段".to_string()))?
            .to_string();

        let dl_path = format!("/api/v1/crates/{}/versions/{}/download",
            version.get("crate_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
            version.get("id").and_then(|v| v.as_u64()).unwrap_or(0)
        );

        let checksum = version.get("checksum")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let yanked = version.get("yanked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(CrateVersion {
            num,
            dl_path,
            checksum,
            yanked,
        })
    }

    fn generate_sample_versions(&self, crate_name: &str) -> Vec<CrateVersion> {
        // 简化的版本生成逻辑
        match crate_name {
            "h2" => vec![
                CrateVersion {
                    num: "0.4.6".to_string(),
                    dl_path: "/api/v1/crates/h2/0.4.6/download".to_string(),
                    checksum: "sample_checksum_h2_0_4_6".to_string(),
                    yanked: false,
                },
                CrateVersion {
                    num: "0.3.26".to_string(),
                    dl_path: "/api/v1/crates/h2/0.3.26/download".to_string(),
                    checksum: "sample_checksum_h2_0_3_26".to_string(),
                    yanked: false,
                },
            ],
            "tokio" => vec![
                CrateVersion {
                    num: "1.40.0".to_string(),
                    dl_path: "/api/v1/crates/tokio/1.40.0/download".to_string(),
                    checksum: "sample_checksum_tokio_1_40_0".to_string(),
                    yanked: false,
                },
                CrateVersion {
                    num: "1.39.3".to_string(),
                    dl_path: "/api/v1/crates/tokio/1.39.3/download".to_string(),
                    checksum: "sample_checksum_tokio_1_39_3".to_string(),
                    yanked: false,
                },
            ],
            "serde" => vec![
                CrateVersion {
                    num: "1.0.210".to_string(),
                    dl_path: "/api/v1/crates/serde/1.0.210/download".to_string(),
                    checksum: "sample_checksum_serde_1_0_210".to_string(),
                    yanked: false,
                },
                CrateVersion {
                    num: "1.0.200".to_string(),
                    dl_path: "/api/v1/crates/serde/1.0.200/download".to_string(),
                    checksum: "sample_checksum_serde_1_0_200".to_string(),
                    yanked: false,
                },
            ],
            _ => vec![],
        }
    }

    /// 根据版本范围选择合适的版本
    pub fn select_version_for_range<'a>(
        &self,
        versions: &'a [CrateVersion],
        range: &str,
    ) -> Option<&'a CrateVersion> {
        // 改进的版本匹配逻辑
        versions.iter().find(|v| {
            !v.yanked && (
                // 1. 精确匹配
                v.num == range ||
                // 2. 前缀匹配（用于版本范围）
                v.num.starts_with(range) ||
                // 3. 主版本号匹配（如 "1" 匹配 "1.x.x"）
                (range.chars().filter(|&c| c == '.').count() == 0 && v.num.starts_with(&format!("{}.", range))) ||
                // 4. 主次版本号匹配（如 "1.0" 匹配 "1.0.x"）
                (range.chars().filter(|&c| c == '.').count() == 1 && v.num.starts_with(&format!("{}.", range)))
            )
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP错误: {0} - {1}")]
    HttpError(u32, String),

    #[error("下载失败: {0}")]
    DownloadFailed(u32, String),

    #[error("解析错误: {0}")]
    ParseError(String),

    #[error("无效的文件格式: {0}")]
    InvalidFileFormat(String),

    #[error("IO错误: {0}")]
    IoError(String),

    #[error("curl错误: {0}")]
    CurlError(#[from] curl::Error),

    #[error("JSON解析错误: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("UTF8转换错误: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_api_client_creation() {
        let config = Config::default();
        let client = CratesApiClient::new(&config);

        assert_eq!(client.user_agent, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        assert_eq!(client.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_version_selection() {
        let config = Config::default();
        let client = CratesApiClient::new(&config);

        let versions = vec![
            CrateVersion {
                num: "1.0.0".to_string(),
                dl_path: "/test".to_string(),
                checksum: "test".to_string(),
                yanked: false,
            },
            CrateVersion {
                num: "1.1.0".to_string(),
                dl_path: "/test".to_string(),
                checksum: "test".to_string(),
                yanked: false,
            },
        ];

        let selected = client.select_version_for_range(&versions, "1.0");
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().num, "1.0.0");
    }
}