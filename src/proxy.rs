use crate::cache::CacheManager;
use crate::config::Config;
use crate::curl_client::{CurlClient, CurlError};
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::{CONTENT_TYPE, CONTENT_LENGTH};
use hyper::service::Service;
use hyper::{Method, Request, Response, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ProxyError {
    #[error("缓存错误: {0}")]
    CacheError(#[from] crate::cache::CacheError),
    #[error("curl错误: {0}")]
    CurlError(#[from] CurlError),
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
    curl_client: Arc<CurlClient>,
    upstream_url: Url,
}

impl ProxyService {
    pub fn new(config: &Config) -> Result<Self, ProxyError> {
        let cache_manager = Arc::new(CacheManager::new(
            &config.cache.storage_path,
            config.cache.default_ttl,
        )?);

        let proxy_url = config.upstream.as_ref()
            .and_then(|u| u.proxy_url.clone());

        let curl_client = Arc::new(CurlClient::new(
            config.user_agent.value.clone(),
            proxy_url,
        ));

        let upstream_url = Url::parse("https://crates.io/")?;

        Ok(Self {
            cache_manager,
            curl_client,
            upstream_url,
        })
    }

    fn parse_crates_request(&self, uri: &Uri) -> Result<(String, String, String), ProxyError> {
        let path = uri.path();

        // 解析crates.io路径格式: /api/v1/crates/{crate_name}/{version}/download
        let parts: Vec<&str> = path.split('/').collect();

        if parts.len() < 6 || parts[0] != "" || parts[1] != "api" || parts[2] != "v1" || parts[3] != "crates" {
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
            "crate.tar.gz"
        } else {
            parts.last().unwrap_or(&"index.json")
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
    ) -> Result<Response<Full<Bytes>>, ProxyError> {
        // 检查缓存
        if self.cache_manager.is_cached(&crate_name, &version, &filename) {
            log::info!("缓存命中: {}-{}-{}", crate_name, version, filename);
            let content = self.cache_manager.get_cached_content(&crate_name, &version, &filename)?;

            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, "application/octet-stream")
                .header(CONTENT_LENGTH, content.len())
                .body(Full::new(Bytes::from(content)))?);
        }

        log::info!("缓存未命中，从上游获取: {}-{}-{}", crate_name, version, filename);

        // 从上游获取
        let upstream_url = self.build_upstream_url(&crate_name, &version, &filename)?;
        let content = self.curl_client.get(upstream_url.as_str())?;

        // 保存到缓存
        self.cache_manager.save_to_cache(&crate_name, &version, &filename, &content)?;

        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/octet-stream")
            .header(CONTENT_LENGTH, content.len())
            .body(Full::new(Bytes::from(content)))?)
    }

    async fn handle_request(&self, req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, ProxyError> {
        let method = req.method();
        let uri = req.uri();

        log::info!("处理请求: {} {}", method, uri);

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
                log::error!("请求解析失败: {}", e);
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from("Bad Request")))?);
            }
        };

        self.handle_crates_request(crate_name, version, filename).await
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

    log::info!("服务器启动，监听地址: {}", config.server.bind_addr);

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        log::info!("新连接来自: {}", remote_addr);

        let service = service.clone();

        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let http = hyper::server::conn::http1::Builder::new();

            if let Err(err) = http.serve_connection(io, service).await {
                log::error!("服务连接错误: {}", err);
            }
        });
    }
}