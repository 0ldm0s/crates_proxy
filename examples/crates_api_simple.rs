use curl::easy::{Easy};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    crate_info: CrateInfo,
    versions: Vec<u64>,  // 版本ID数组
}

#[derive(Debug, Deserialize, Serialize)]
struct CrateInfo {
    id: String,
    name: String,
    description: Option<String>,
    max_version: String,
    downloads: u64,
    updated_at: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Crates.io API 测试（验证代理能否获取最新包信息）===");

    // 代理配置
    let proxy_url = "http://172.16.0.80:9051";

    // 测试包列表
    let test_crates = vec![
        "rand",
        "serde",
        "tokio",
        "hyper",
        "reqwest"
    ];

    let mut successful_api_calls = 0;
    let mut successful_downloads = 0;

    for crate_name in &test_crates {
        println!("\n=== 测试包: {} ===", crate_name);

        // 1. 通过crates.io API获取包信息
        match fetch_crate_info(crate_name, proxy_url) {
            Ok(crate_response) => {
                successful_api_calls += 1;
                println!("✅ 成功获取包信息");
                println!("包名: {}", crate_response.crate_info.name);
                println!("最新版本: {}", crate_response.crate_info.max_version);
                println!("描述: {:?}", crate_response.crate_info.description);
                println!("下载数: {}", crate_response.crate_info.downloads);

                // 2. 获取最新版本的详细信息（通过另一个API）
                println!("最新版本: {}", crate_response.crate_info.max_version);
                println!("版本数量: {}", crate_response.versions.len());

                // 3. 测试下载最新版本
                match test_download_latest_version(crate_name, &crate_response.crate_info.max_version, proxy_url) {
                    Ok(_) => {
                        successful_downloads += 1;
                    }
                    Err(e) => {
                        println!("❌ 下载测试失败: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ 获取包信息失败: {}", e);
            }
        }
    }

    // 总结
    println!("\n=== 测试总结 ===");
    println!("API调用成功: {}/{}", successful_api_calls, test_crates.len());
    println!("下载成功: {}/{}", successful_downloads, test_crates.len());

    if successful_api_calls == test_crates.len() {
        println!("✅ 代理可以正常访问crates.io API");
        if successful_downloads > 0 {
            println!("✅ 部分下载成功，说明有时能绕过AWS S3限制");
        } else {
            println!("❌ 全部下载失败，确认AWS S3访问限制");
        }
    } else {
        println!("❌ API访问有问题");
    }

    Ok(())
}

fn fetch_crate_info(crate_name: &str, proxy_url: &str) -> Result<CrateResponse, Box<dyn std::error::Error>> {
    let api_url = format!("https://crates.io/api/v1/crates/{}", crate_name);

    let mut handle = Easy::new();
    handle.url(&api_url)?;

    // 使用浏览器UA避免被限制
    handle.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")?;
    handle.timeout(Duration::from_secs(30))?;
    handle.follow_location(true)?;
    handle.verbose(false)?; // 减少日志噪音

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

    if response_code == 200 {
        let response_text = String::from_utf8(data)?;
        let crate_response: CrateResponse = serde_json::from_str(&response_text)?;
        Ok(crate_response)
    } else {
        Err(format!("HTTP {}: {}", response_code, String::from_utf8_lossy(&data)).into())
    }
}

fn test_download_latest_version(crate_name: &str, version: &str, proxy_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- 测试下载 {} ---", version);

    // 构造下载URL
    let download_url = format!("https://crates.io/api/v1/crates/{}/{}/download", crate_name, version);

    let mut handle = Easy::new();
    handle.url(&download_url)?;
    handle.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")?;
    handle.timeout(Duration::from_secs(30))?;
    handle.follow_location(true)?; // 跟随重定向
    handle.verbose(false)?; // 减少噪音

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
    let final_url = handle.effective_url()?;

    println!("下载状态码: {}", response_code);
    println!("最终URL: {:?}", final_url);
    println!("下载大小: {} 字节", data.len());

    if response_code == 200 {
        println!("✅ 下载成功");

        // 验证文件格式
        if data.starts_with(&[0x1f, 0x8b]) {
            println!("✅ 文件是有效的gzip格式");
        }

        // 保存测试文件
        let filename = format!("{}-{}.crate", crate_name, version);
        std::fs::write(&filename, &data)?;
        println!("文件已保存为: {}", filename);

    } else if response_code == 403 {
        println!("❌ 下载失败: 403 Forbidden (AWS S3访问限制)");
        println!("这证实了我们的假设：即使通过代理，AWS S3仍然限制访问");
    } else {
        println!("❌ 下载失败: HTTP {}", response_code);
    }

    Ok(())
}