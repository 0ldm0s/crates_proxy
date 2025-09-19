use curl::easy::{Easy};
use serde_json::Value;
use std::io::{self, Write};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 简单API验证测试 ===");

    // 代理配置
    let proxy_url = "http://172.16.0.80:9051";

    // 测试包列表
    let test_crates = vec![
        "rand",
        "serde",
        "tokio",
    ];

    for crate_name in test_crates {
        println!("\n=== 测试包: {} ===", crate_name);

        // 通过crates.io API获取包信息
        match test_api_access(crate_name, proxy_url) {
            Ok(success) => {
                if success {
                    println!("✅ API访问成功");

                    // 尝试下载
                    match test_download_access(crate_name, proxy_url) {
                        Ok(download_success) => {
                            if download_success {
                                println!("✅ 下载测试成功");
                            } else {
                                println!("❌ 下载测试失败 (403)");
                            }
                        }
                        Err(e) => {
                            println!("❌ 下载测试出错: {}", e);
                        }
                    }
                } else {
                    println!("❌ API访问失败");
                }
            }
            Err(e) => {
                println!("❌ API测试出错: {}", e);
            }
        }
    }

    Ok(())
}

fn test_api_access(crate_name: &str, proxy_url: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let api_url = format!("https://crates.io/api/v1/crates/{}", crate_name);

    let mut handle = Easy::new();
    handle.url(&api_url)?;
    handle.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")?;
    handle.timeout(Duration::from_secs(30))?;
    handle.follow_location(true)?;
    handle.verbose(false)?;

    // 设置代理
    handle.proxy(proxy_url)?;

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
    println!("API状态码: {}", response_code);

    if response_code == 200 {
        // 尝试解析JSON，只提取基本信息
        let response_text = String::from_utf8(data)?;
        let json: Value = serde_json::from_str(&response_text)?;

        if let Some(crate_info) = json.get("crate") {
            let name = crate_info.get("name").and_then(|v| v.as_str()).unwrap_or("未知");
            let max_version = crate_info.get("max_version").and_then(|v| v.as_str()).unwrap_or("未知");
            let downloads = crate_info.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0);

            println!("包名: {}", name);
            println!("最新版本: {}", max_version);
            println!("下载数: {}", downloads);

            return Ok(true);
        }
    }

    Ok(false)
}

fn test_download_access(crate_name: &str, proxy_url: &str) -> Result<bool, Box<dyn std::error::Error>> {
    // 先获取最新版本
    let api_url = format!("https://crates.io/api/v1/crates/{}", crate_name);

    let mut handle = Easy::new();
    handle.url(&api_url)?;
    handle.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")?;
    handle.timeout(Duration::from_secs(30))?;
    handle.follow_location(true)?;
    handle.verbose(false)?;
    handle.proxy(proxy_url)?;

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
        return Ok(false);
    }

    // 解析获取最新版本
    let response_text = String::from_utf8(data)?;
    let json: Value = serde_json::from_str(&response_text)?;

    let max_version = match json.get("crate").and_then(|c| c.get("max_version")).and_then(|v| v.as_str()) {
        Some(version) => version,
        None => return Ok(false),
    };

    println!("测试下载版本: {}", max_version);

    // 构造下载URL
    let download_url = format!("https://crates.io/api/v1/crates/{}/{}/download", crate_name, max_version);

    let mut handle2 = Easy::new();
    handle2.url(&download_url)?;
    handle2.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")?;
    handle2.timeout(Duration::from_secs(30))?;
    handle2.follow_location(true)?;
    handle2.verbose(false)?;
    handle2.proxy(proxy_url)?;

    let mut download_data = Vec::new();
    {
        let mut transfer = handle2.transfer();
        transfer.write_function(|buf| {
            download_data.extend_from_slice(buf);
            Ok(buf.len())
        })?;
        transfer.perform()?;
    }

    let download_response_code = handle2.response_code()?;
    println!("下载状态码: {}", download_response_code);

    Ok(download_response_code == 200)
}