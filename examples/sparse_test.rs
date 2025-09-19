use curl::easy::{Easy};
use std::io::{self, Write};
use std::time::Duration;
use url::Url;

#[derive(Debug)]
struct CrateInfo {
    name: String,
    version: String,
    checksum: String,
    dl_path: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Sparse协议解析器测试 ===");

    // 代理配置
    let proxy_url = "http://172.16.0.80:9051";
    let sparse_base_url = "https://index.crates.io/";

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

        // 1. 获取包的sparse索引路径
        let sparse_path = get_sparse_path(crate_name);
        println!("Sparse路径: {}", sparse_path);

        // 2. 通过代理访问sparse索引
        match fetch_sparse_index(sparse_base_url, &sparse_path, proxy_url) {
            Ok(index_data) => {
                println!("✅ 成功获取索引数据 ({} 字节)", index_data.len());

                // 3. 解析索引数据
                match parse_sparse_index(&index_data) {
                    Ok(versions) => {
                        println!("✅ 解析到 {} 个版本", versions.len());

                        // 显示最新版本
                        if let Some(latest) = versions.first() {
                            println!("最新版本: {} ({})", latest.version, latest.checksum);
                            println!("下载路径: {}", latest.dl_path);

                            // 4. 尝试下载该版本的文件
                            test_download_with_proxy(&latest, proxy_url)?;
                        }
                    }
                    Err(e) => {
                        println!("❌ 解析索引失败: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ 获取索引失败: {}", e);
            }
        }
    }

    Ok(())
}

fn get_sparse_path(crate_name: &str) -> String {
    // sparse协议的路径规则：根据RFC 2789，路径是 {first-char}/{second-char}/{crate-name}
    let chars: Vec<char> = crate_name.chars().collect();
    match chars.len() {
        0 => String::new(),
        1 => format!("1/{}", crate_name),
        2 => format!("{}/{}/{}", chars[0], chars[1], crate_name),
        _ => format!("{}/{}/{}", chars[0], chars[1], crate_name)
    }
}

fn fetch_sparse_index(base_url: &str, path: &str, proxy_url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let url = format!("{}{}", base_url, path);
    println!("请求URL: {}", url);

    let mut handle = Easy::new();
    handle.url(&url)?;
    handle.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
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
        Ok(data)
    } else {
        Err(format!("HTTP {}: {}", response_code, String::from_utf8_lossy(&data)).into())
    }
}

fn parse_sparse_index(data: &[u8]) -> Result<Vec<CrateInfo>, Box<dyn std::error::Error>> {
    let mut versions = Vec::new();
    let content = String::from_utf8(data.to_vec())?;

    println!("原始数据:\n{}", content);

    // sparse协议的每行格式：{version} "{checksum}" {dependencies...}
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let version = parts[0].to_string();
            let checksum = parts[1].trim_matches('"').to_string();

            // 构造下载路径（从config.json获取）
            let dl_path = format!("https://static.crates.io/crates/{}/{}/download",
                                 "crate_name_placeholder", &version); // 这里需要包名，暂时用占位符

            versions.push(CrateInfo {
                name: "crate_name_placeholder".to_string(), // 实际使用时需要传入包名
                version,
                checksum,
                dl_path,
            });
        }
    }

    // 按版本号排序（最新的在前）
    versions.sort_by(|a, b| b.version.cmp(&a.version));

    Ok(versions)
}

fn test_download_with_proxy(crate_info: &CrateInfo, proxy_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- 测试下载 {} ---", crate_info.version);

    // 构造正确的下载URL
    let download_url = format!("https://static.crates.io/crates/{}/download",
                             "crate_name_placeholder"); // 实际需要包名

    let mut handle = Easy::new();
    handle.url(&download_url)?;
    handle.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
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
    println!("下载状态码: {}", response_code);
    println!("下载大小: {} 字节", data.len());

    if response_code == 200 {
        println!("✅ 下载成功");

        // 验证文件格式
        if data.starts_with(&[0x1f, 0x8b]) {
            println!("✅ 文件是有效的gzip格式");
        }

        // 验证校验和（如果需要）
        println!("校验和: {}", crate_info.checksum);
    } else {
        println!("❌ 下载失败");

        // 如果官方源失败，尝试国内镜像
        println!("尝试国内镜像...");
        test_china_mirror(crate_info, proxy_url)?;
    }

    Ok(())
}

fn test_china_mirror(crate_info: &CrateInfo, proxy_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mirror_urls = vec![
        "https://mirrors.ustc.edu.cn/crates.io/crates",
        "https://rsproxy.cn/crates",
    ];

    for mirror_base in mirror_urls {
        println!("测试镜像: {}", mirror_base);

        let mirror_url = format!("{}/{}/download", mirror_base, "crate_name_placeholder");

        let mut handle = Easy::new();
        handle.url(&mirror_url)?;
        handle.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
        handle.timeout(Duration::from_secs(30))?;
        handle.follow_location(true)?;
        handle.verbose(false)?; // 减少日志噪音

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
            println!("✅ 镜像 {} 下载成功 ({} 字节)", mirror_base, data.len());
            return Ok(());
        } else {
            println!("❌ 镜像 {} 失败: {}", mirror_base, response_code);
        }
    }

    Ok(())
}