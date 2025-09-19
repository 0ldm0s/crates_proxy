use crate::cache::CacheManager;
use crate::config::Config;
use crate::crates_api::{CratesApiClient, CrateVersion};
use crate::curl_client::{CurlClient, CurlError};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH};
use hyper::service::Service;
use hyper::{Method, Request, Response, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("缓存错误: {0}")]
    CacheError(#[from] crate::cache::CacheError),
    #[error("curl错误: {0}")]
    CurlError(#[from] CurlError),
    #[error("API错误: {0}")]
    ApiError(#[from] crate::crates_api::ApiError),
    #[error("URL解析错误: {0}")]
    UrlError(#[from] url::ParseError),
    #[error("超文本传输协议错误: {0}")]
    HyperError(#[from] hyper::Error),
    #[error("HTTP错误: {0}")]
    HttpError(#[from] hyper::http::Error),
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
    #[error("无效的请求: {0}")]
    InvalidRequest(String),
}

#[derive(Clone)]
pub struct ProxyService {
    cache_manager: Arc<CacheManager>,
    api_client: Arc<CratesApiClient>,
    curl_client: Arc<CurlClient>,
    upstream_url: Url,
    latest_versions: Arc<RwLock<HashMap<String, String>>>, // 包名 -> 最新版本号的映射
}

impl ProxyService {
    pub fn new(config: &Config) -> Result<Self, ProxyError> {
        rat_logger::info!("创建ProxyService...");
        rat_logger::info!("缓存路径: {}", config.cache.storage_path);
        rat_logger::info!("User-Agent: {}", config.user_agent.value);

        let cache_manager = Arc::new(CacheManager::new(
            &config.cache.storage_path,
            config.cache.default_ttl,
        )?);

        let api_client = Arc::new(CratesApiClient::new(config));
        rat_logger::info!("CratesApiClient创建成功");

        let proxy_url = config.upstream.as_ref()
            .and_then(|u| u.proxy_url.clone());

        rat_logger::info!("上游代理: {:?}", proxy_url);

        let curl_client = Arc::new(CurlClient::new(
            config.user_agent.value.clone(),
            proxy_url,
        ));

        rat_logger::info!("CurlClient创建成功");

        let upstream_url = Url::parse("https://crates.io/")?;

        rat_logger::info!("ProxyService创建成功");

        Ok(Self {
            cache_manager,
            api_client,
            curl_client,
            upstream_url,
            latest_versions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 获取并缓存最新版本号
    fn get_latest_version(&self, crate_name: &str) -> Result<String, ProxyError> {
        // 首先检查内存缓存
        {
            let versions = self.latest_versions.read().unwrap();
            if let Some(version) = versions.get(crate_name) {
                rat_logger::info!("从缓存获取最新版本: {} -> {}", crate_name, version);
                return Ok(version.clone());
            }
        }

        // 缓存未命中，从API获取
        rat_logger::info!("从API获取最新版本: {}", crate_name);
        let info = self.api_client.get_crate_info(crate_name)
            .map_err(|e| ProxyError::ApiError(e))?;

        let latest_version = info.max_version.clone();

        // 更新缓存
        {
            let mut versions = self.latest_versions.write().unwrap();
            versions.insert(crate_name.to_string(), latest_version.clone());
            rat_logger::info!("更新最新版本缓存: {} -> {}", crate_name, latest_version);
        }

        Ok(latest_version)
    }

    fn parse_crates_request(&self, uri: &Uri) -> Result<(String, String, String), ProxyError> {
        let path = uri.path();
        rat_logger::info!("解析请求路径: {}", path);

        // 解析crates.io路径格式: /api/v1/crates/{crate_name}/{version}/download
        let parts: Vec<&str> = path.split('/').collect();
        rat_logger::info!("路径分割: {:?}", parts);

        if parts.len() < 6 || parts[0] != "" || parts[1] != "api" || parts[2] != "v1" || parts[3] != "crates" {
            rat_logger::error!("路径验证失败: 长度={}, parts={:?}", parts.len(), parts);
            return Err(ProxyError::InvalidRequest(
                "无效的crates请求路径".to_string(),
            ));
        }

        let crate_name = parts[4];
        let version = if parts.len() > 5 && parts[5] != "download" {
            parts[5]
        } else {
            "latest"
        };

        let filename = if parts.last() == Some(&"download") {
            format!("{}-{}.crate", crate_name, version)
        } else {
            parts.last().unwrap_or(&"index.json").to_string()
        };

        Ok((crate_name.to_string(), version.to_string(), filename.to_string()))
    }

    fn build_upstream_url(&self, crate_name: &str, version: &str, filename: &str) -> Result<Url, ProxyError> {
        let mut url = self.upstream_url.clone();

        if filename == "crate.tar.gz" {
            url.path_segments_mut()
                .map_err(|_| ProxyError::InvalidRequest("URL路径错误".to_string()))?
                .push("api")
                .push("v1")
                .push("crates")
                .push(crate_name)
                .push(version)
                .push("download");
        } else {
            url.path_segments_mut()
                .map_err(|_| ProxyError::InvalidRequest("URL路径错误".to_string()))?
                .push("api")
                .push("v1")
                .push("crates")
                .push(crate_name);
        }

        Ok(url)
    }

    async fn handle_crates_request(
        &self,
        crate_name: String,
        version: String,
        filename: String,
        original_path: String,
    ) -> Result<Response<Full<Bytes>>, ProxyError> {
        // 智能版本处理
        let actual_version = if version == "latest" {
            // 获取最新版本（使用缓存）
            match self.get_latest_version(&crate_name) {
                Ok(version) => {
                    rat_logger::info!("获取到最新版本: {}", version);
                    version
                }
                Err(e) => {
                    rat_logger::error!("获取包信息失败: {}", e);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Full::new(Bytes::from(format!("获取包信息失败: {}", e))))?);
                }
            }
        } else {
            // 验证请求的版本是否存在
            match self.api_client.get_available_versions(&crate_name) {
                Ok(versions) => {
                    if let Some(selected_version) = self.api_client.select_version_for_range(&versions, &version) {
                        rat_logger::info!("选择版本: {}", selected_version.num);
                        selected_version.num.clone()
                    } else {
                        rat_logger::error!("未找到匹配版本: {}", version);
                        return Ok(Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(Full::new(Bytes::from(format!("版本 {} 不存在", version))))?);
                    }
                }
                Err(e) => {
                    rat_logger::error!("获取版本列表失败: {}", e);
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Full::new(Bytes::from(format!("获取版本列表失败: {}", e))))?);
                }
            }
        };

        // 构造缓存键
        let cache_filename = if filename.ends_with(".crate") {
            format!("{}-{}.crate", crate_name, actual_version)
        } else {
            filename.clone()
        };

        // 检查缓存（使用实际版本）
        if self.cache_manager.is_cached(&crate_name, &actual_version, &cache_filename) {
            rat_logger::info!("缓存命中: {}-{}-{}", crate_name, actual_version, cache_filename);
            let content = self.cache_manager.get_cached_content(&crate_name, &actual_version, &cache_filename)?;

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(CONTENT_LENGTH, content.len())
                .body(Full::new(Bytes::from(content)))?);
        }

        rat_logger::info!("缓存未命中，从上游获取: {}-{}-{}", crate_name, actual_version, cache_filename);

        // 下载文件
        let cache_path = self.cache_manager.get_cache_path(&crate_name, &actual_version, &cache_filename);
        rat_logger::info!("下载文件到: {:?}", cache_path);

        match self.api_client.download_crate_version(&crate_name, &actual_version, &cache_path) {
            Ok(_) => {
                rat_logger::info!("下载成功: {}-{}", crate_name, actual_version);

                // 从缓存读取内容
                let content = self.cache_manager.get_cached_content(&crate_name, &actual_version, &cache_filename)?;

                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "application/octet-stream")
                    .header(CONTENT_LENGTH, content.len())
                    .body(Full::new(Bytes::from(content)))?)
            }
            Err(e) => {
                rat_logger::error!("下载失败: {}", e);
                Ok(Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Full::new(Bytes::from(format!("下载失败: {}", e))))?)
            }
        }
    }

    async fn handle_request(&self, req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, ProxyError> {
        let method = req.method();
        let uri = req.uri();

        rat_logger::info!("处理请求: {} {}", method, uri);

        // 只支持GET请求
        if *method != Method::GET {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Full::new(Bytes::from("Method Not Allowed")))?);
        }

        // 解析crates请求
        let (crate_name, version, filename) = match self.parse_crates_request(uri) {
            Ok(parsed) => parsed,
            Err(e) => {
                rat_logger::error!("请求解析失败: {}", e);
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from("Bad Request")))?);
            }
        };

        let original_path = uri.path().to_string();
        self.handle_crates_request(crate_name, version, filename, original_path).await
    }
}

impl Service<Request<hyper::body::Incoming>> for ProxyService {
    type Response = Response<Full<Bytes>>;
    type Error = ProxyError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<hyper::body::Incoming>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { this.handle_request(req).await })
    }
}

pub async fn run_server(config: &Config) -> Result<(), ProxyError> {
    let service = ProxyService::new(config)?;

    let listener = tokio::net::TcpListener::bind(&config.server.bind_addr).await?;

    rat_logger::info!("服务器启动，监听地址: {}", config.server.bind_addr);

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        rat_logger::info!("新连接来自: {}", remote_addr);

        let service = service.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let http = hyper::server::conn::http1::Builder::new();

            if let Err(err) = http.serve_connection(io, service).await {
                rat_logger::error!("服务连接错误: {}", err);
            }
        });
    }
}