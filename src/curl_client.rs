use curl::easy::{Easy, List};
use std::io::{self, Read};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CurlError {
    #[error("curl错误: {0}")]
    CurlError(#[from] curl::Error),
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
    #[error("HTTP错误: {0}")]
    HttpError(String),
    #[error("超时错误")]
    TimeoutError,
}

pub struct CurlClient {
    user_agent: String,
    proxy_url: Option<String>,
    timeout: Duration,
}

impl CurlClient {
    pub fn new(user_agent: String, proxy_url: Option<String>) -> Self {
        Self {
            user_agent,
            proxy_url,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn get(&self, url: &str) -> Result<Vec<u8>, CurlError> {
        let mut handle = Easy::new();
        handle.url(url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;

        // 设置代理
        if let Some(ref proxy) = self.proxy_url {
            handle.proxy(proxy)?;
        }

        // 设置重定向跟随
        handle.follow_location(true)?;
        handle.max_redirections(5)?;

        // 创建缓冲区来存储响应
        let mut buf = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer.write_function(|data| {
                buf.extend_from_slice(data);
                Ok(data.len())
            })?;

            transfer.perform()?;
        }

        // 检查HTTP状态码
        let response_code = handle.response_code()?;
        if response_code >= 400 {
            return Err(CurlError::HttpError(format!(
                "HTTP {}: {}",
                response_code,
                handle.response_code().unwrap_or(0)
            )));
        }

        Ok(buf)
    }

    pub fn download_file(&self, url: &str, output_path: &str) -> Result<(), CurlError> {
        let mut handle = Easy::new();
        handle.url(url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;

        // 设置代理
        if let Some(ref proxy) = self.proxy_url {
            handle.proxy(proxy)?;
        }

        // 设置重定向跟随
        handle.follow_location(true)?;
        handle.max_redirections(5)?;

        // 创建输出文件
        let mut file = std::fs::File::create(output_path)?;

        {
            let mut transfer = handle.transfer();
            transfer.write_function(|data| {
                use std::io::Write;
                file.write_all(data).map_err(|e| {
                    curl::easy::WriteError::Pause
                })?;
                Ok(data.len())
            })?;

            transfer.perform()?;
        }

        // 检查HTTP状态码
        let response_code = handle.response_code()?;
        if response_code >= 400 {
            return Err(CurlError::HttpError(format!(
                "HTTP {}: {}",
                response_code,
                handle.response_code().unwrap_or(0)
            )));
        }

        Ok(())
    }

    pub fn head(&self, url: &str) -> Result<u32, CurlError> {
        let mut handle = Easy::new();
        handle.url(url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;
        handle.nobody(true)?;

        // 设置代理
        if let Some(ref proxy) = self.proxy_url {
            handle.proxy(proxy)?;
        }

        // 设置重定向跟随
        handle.follow_location(true)?;
        handle.max_redirections(5)?;

        handle.perform()?;

        let response_code = handle.response_code()?;
        Ok(response_code)
    }

    pub fn set_headers(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>, CurlError> {
        let mut handle = Easy::new();
        handle.url(url)?;
        handle.useragent(&self.user_agent)?;
        handle.timeout(self.timeout)?;

        // 设置代理
        if let Some(ref proxy) = self.proxy_url {
            handle.proxy(proxy)?;
        }

        // 设置自定义头
        let mut header_list = List::new();
        for (key, value) in headers {
            header_list.append(&format!("{}: {}", key, value))?;
        }
        handle.http_headers(header_list)?;

        // 设置重定向跟随
        handle.follow_location(true)?;
        handle.max_redirections(5)?;

        let mut buf = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer.write_function(|data| {
                buf.extend_from_slice(data);
                Ok(data.len())
            })?;

            transfer.perform()?;
        }

        // 检查HTTP状态码
        let response_code = handle.response_code()?;
        if response_code >= 400 {
            return Err(CurlError::HttpError(format!(
                "HTTP {}: {}",
                response_code,
                handle.response_code().unwrap_or(0)
            )));
        }

        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = CurlClient::new(
            "test-agent".to_string(),
            Some("http://proxy.example.com:8080".to_string()),
        );

        assert_eq!(client.user_agent, "test-agent");
        assert_eq!(client.proxy_url, Some("http://proxy.example.com:8080".to_string()));
    }
}