use curl::easy::{Easy};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    crate_info: CrateInfo,
    versions: Vec<VersionInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CrateInfo {
    id: String,
    name: String,
    description: Option<String>,
    max_version: String,
    downloads: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct VersionInfo {
    id: String,
    num: String,
    dl_path: String,
    checksum: String,
    yanked: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Crates.io API 测试 ===");

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

    for crate_name in test_crates {
        println!("\n=== 测试包: {} ===", crate_name);

        // 1. 通过crates.io API获取包信息
        match fetch_crate_info(crate_name, proxy_url) {
            Ok(crate_response) => {
                println!("✅ 成功获取包信息");
                println!("包名: {}", crate_response.crate_info.name);
                println!("最新版本: {}", crate_response.crate_info.max_version);
                println!("描述: {:?}", crate_response.crate_info.description);
                println!("下载数: {}", crate_response.crate_info.downloads);

                // 2. 获取最新版本的详细信息
                if let Some(latest_version) = crate_response.versions.first() {
                    println!("\n--- 最新版本详情 ---");
                    println!("版本号: {}", latest_version.num);
                    println!("下载路径: {}", latest_version.dl_path);
                    println!("校验和: {}", latest_version.checksum);
                    println!("是否撤销: {}", latest_version.yanked);

                    // 3. 测试下载
                    test_version_download(latest_version, proxy_url, crate_name)?;
                }
            }
            Err(e) => {
                println!("❌ 获取包信息失败: {}", e);
            }
        }
    }

    Ok(())
}

fn fetch_crate_info(crate_name: &str, proxy_url: &str) -> Result<CrateResponse, Box<dyn std::error::Error>> {
    let api_url = format!("https://crates.io/api/v1/crates/{}", crate_name);
    println!("API URL: {}", api_url);

    let mut handle = Easy::new();
    handle.url(&api_url)?;

    // 使用浏览器UA避免被限制
    handle.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")?;
    handle.timeout(Duration::from_secs(30))?;
    handle.follow_location(true)?;
    handle.verbose(true)?;

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
    println!("响应状态码: {}", response_code);

    if response_code == 200 {
        let response_text = String::from_utf8(data)?;
        println!("响应大小: {} 字符", response_text.len());

        // 解析JSON
        let crate_response: CrateResponse = serde_json::from_str(&response_text)?;
        Ok(crate_response)
    } else {
        Err(format!("HTTP {}: {}", response_code, String::from_utf8_lossy(&data)).into())
    }
}

fn test_version_download(version: &VersionInfo, proxy_url: &str, crate_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- 测试下载 {} ---", version.num);

    // 构造下载URL
    let download_url = format!("https://crates.io/api/v1/crates/{}/{}/download", crate_name, version.num);
    println!("下载URL: {}", download_url);

    let mut handle = Easy::new();
    handle.url(&download_url)?;
    handle.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0. Safari/537.36")?;
    handle.timeout(Duration::from_secs(30))?;
    handle.follow_location(true)?; // 跟随重定向
    handle.verbose(true)?;

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

    println!("最终URL: {:?}", final_url);
    println!("下载状态码: {}", response_code);
    println!("下载大小: {} 字节", data.len());

    if response_code == 200 {
        println!("✅ 下载成功");

        // 验证文件格式
        if data.starts_with(&[0x1f, 0x8b]) {
            println!("✅ 文件是有效的gzip格式");
        } else {
            println!("⚠️  文件格式异常");
        }

        // 保存测试文件
        let filename = format!("{}-{}.crate", crate_name, version.num);
        std::fs::write(&filename, &data)?;
        println!("文件已保存为: {}", filename);

        // 验证校验和（如果需要）
        println!("预期校验和: {}", version.checksum);

    } else if response_code == 403 {
        println!("❌ 下载失败: 403 Forbidden");

        // 尝试国内镜像
        println!("尝试国内镜像下载...");
        test_china_mirror_download(crate_name, version.num, proxy_url)?;
    } else {
        println!("❌ 下载失败: HTTP {}", response_code);
    }

    Ok(())
}

fn test_china_mirror_download(crate_name: &str, version: &str, proxy_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mirrors = vec![
        ("中科大镜像", "https://mirrors.ustc.edu.cn/crates.io/crates"),
        ("rsproxy镜像", "https://rsproxy.cn/crates"),
    ];

    for (mirror_name, mirror_base) in mirrors {
        println!("测试{}:", mirror_name);

        let mirror_url = format!("{}/{}/{}-{}.crate", mirror_base, crate_name, crate_name, version);
        println!("镜像URL: {}", mirror_url);

        let mut handle = Easy::new();
        handle.url(&mirror_url)?;
        handle.useragent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")?;
        handle.timeout(Duration::from_secs(30))?;
        handle.follow_location(false)?; // 镜像通常不重定向
        handle.verbose(false)?; // 减少噪音

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
            println!("✅ {} 下载成功 ({} 字节)", mirror_name, data.len());

            // 保存文件
            let filename = format!("{}-{}_from_{}.crate", crate_name, version, mirror_name);
            std::fs::write(&filename, &data)?;
            println!("已保存为: {}", filename);

            return Ok(());
        } else {
            println!("❌ {} 失败: HTTP {}", mirror_name, response_code);
        }
    }

    Ok(())
}