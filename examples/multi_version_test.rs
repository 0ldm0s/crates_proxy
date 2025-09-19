use curl::easy::{Easy};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Duration;
use rand::Rng;

#[derive(Debug)]
struct VersionInfo {
    num: String,
    yanked: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 多版本下载场景测试 ===");

    // 代理配置
    let proxy_url = "http://172.16.0.80:9051";

    // 模拟项目依赖场景
    let test_scenarios = vec![
        ("h2", vec!["0.3", "0.4"]),  // 模拟不同包依赖不同版本的h2
        ("tokio", vec!["1.0", "1.40"]), // 模拟新旧版本共存
        ("serde", vec!["1.0", "1.0.200"]), // 模拟小版本差异
    ];

    for (crate_name, version_ranges) in test_scenarios {
        println!("\n=== 模拟场景: {} 多版本依赖 ===", crate_name);

        // 1. 获取包的所有版本
        match get_all_versions(crate_name, proxy_url) {
            Ok(versions) => {
                println!("✅ 获取到 {} 个版本", versions.len());

                // 2. 为每个版本范围选择合适的版本
                for range in version_ranges {
                    println!("\n--- 寻找匹配版本范围: {} ---", range);

                    if let Some(selected_version) = select_version_for_range(&versions, range) {
                        println!("✅ 选择版本: {}", selected_version.num);

                        // 3. 测试下载选中的版本
                        match test_download_version(crate_name, &selected_version.num, proxy_url) {
                            Ok(success) => {
                                if success {
                                    println!("✅ 版本 {} 下载成功", selected_version.num);
                                } else {
                                    println!("❌ 版本 {} 下载失败", selected_version.num);
                                }
                            }
                            Err(e) => {
                                println!("❌ 下载出错: {}", e);
                            }
                        }
                    } else {
                        println!("❌ 未找到匹配版本范围 {} 的版本", range);
                    }
                }

                // 4. 额外测试：随机选择一个版本
                if !versions.is_empty() {
                    let random_index = rand::thread_rng().gen_range(0..versions.len());
                    let random_version = &versions[random_index];
                    println!("\n--- 随机测试版本: {} ---", random_version.num);

                    match test_download_version(crate_name, &random_version.num, proxy_url) {
                        Ok(success) => {
                            if success {
                                println!("✅ 随机版本 {} 下载成功", random_version.num);
                            } else {
                                println!("❌ 随机版本 {} 下载失败", random_version.num);
                            }
                        }
                        Err(e) => {
                            println!("❌ 随机下载出错: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ 获取版本列表失败: {}", e);
            }
        }
    }

    // 5. 模拟复杂的依赖树场景
    println!("\n=== 模拟复杂依赖树场景 ===");
    simulate_complex_dependency_tree(proxy_url)?;

    Ok(())
}

fn get_all_versions(crate_name: &str, proxy_url: &str) -> Result<Vec<VersionInfo>, Box<dyn std::error::Error>> {
    // crates.io API 不直接提供所有版本列表，我们需要使用版本详情API
    // 先获取基本信息，然后通过其他方式获取版本列表

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
        return Err(format!("API请求失败: HTTP {}", response_code).into());
    }

    let response_text = String::from_utf8(data)?;
    let json: Value = serde_json::from_str(&response_text)?;

    // 从versions数组中获取版本ID列表
    let mut versions = Vec::new();

    if let Some(version_ids) = json.get("versions").and_then(|v| v.as_array()) {
        println!("发现 {} 个版本ID", version_ids.len());

        // 为了演示，我们构造一些常见的版本号
        // 在实际实现中，需要通过版本详情API获取真实的版本信息
        let sample_versions = generate_sample_versions(crate_name);
        versions.extend(sample_versions);
    }

    Ok(versions)
}

fn generate_sample_versions(crate_name: &str) -> Vec<VersionInfo> {
    // 为演示目的生成一些示例版本
    match crate_name {
        "h2" => vec![
            VersionInfo { num: "0.3.26".to_string(), yanked: false },
            VersionInfo { num: "0.4.6".to_string(), yanked: false },
            VersionInfo { num: "0.4.5".to_string(), yanked: false },
            VersionInfo { num: "0.4.4".to_string(), yanked: false },
            VersionInfo { num: "0.3.25".to_string(), yanked: false },
            VersionInfo { num: "0.3.24".to_string(), yanked: false },
        ],
        "tokio" => vec![
            VersionInfo { num: "1.40.0".to_string(), yanked: false },
            VersionInfo { num: "1.39.3".to_string(), yanked: false },
            VersionInfo { num: "1.38.0".to_string(), yanked: false },
            VersionInfo { num: "1.37.0".to_string(), yanked: false },
            VersionInfo { num: "1.0.0".to_string(), yanked: false },
            VersionInfo { num: "0.2.21".to_string(), yanked: false },
        ],
        "serde" => vec![
            VersionInfo { num: "1.0.210".to_string(), yanked: false },
            VersionInfo { num: "1.0.205".to_string(), yanked: false },
            VersionInfo { num: "1.0.200".to_string(), yanked: false },
            VersionInfo { num: "1.0.190".to_string(), yanked: false },
            VersionInfo { num: "1.0.0".to_string(), yanked: false },
            VersionInfo { num: "0.9.15".to_string(), yanked: false },
        ],
        _ => vec![
            VersionInfo { num: "1.0.0".to_string(), yanked: false },
            VersionInfo { num: "0.1.0".to_string(), yanked: false },
        ],
    }
}

fn select_version_for_range<'a>(versions: &'a [VersionInfo], range: &str) -> Option<&'a VersionInfo> {
    // 简化的版本匹配逻辑
    versions.iter().find(|v| {
        !v.yanked && v.num.starts_with(range)
    })
}

fn test_download_version(crate_name: &str, version: &str, proxy_url: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let download_url = format!("https://crates.io/api/v1/crates/{}/{}/download", crate_name, version);

    let mut handle = Easy::new();
    handle.url(&download_url)?;
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

    if response_code == 200 {
        // 保存文件用于验证
        let filename = format!("{}-{}.crate", crate_name, version);
        std::fs::write(&filename, &data)?;
        println!("文件已保存: {} ({} 字节)", filename, data.len());
    }

    Ok(response_code == 200)
}

fn simulate_complex_dependency_tree(proxy_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("模拟复杂项目依赖树:");

    // 模拟一个项目的依赖树
    let dependencies = vec![
        ("tokio", "1.39"),
        ("serde", "1.0.200"),
        ("hyper", "1.4"),
        ("h2", "0.4"),
        ("tracing", "0.1"),
    ];

    let mut cache = HashMap::new(); // 模拟本地缓存

    for (dep_name, version_range) in dependencies {
        println!("\n处理依赖: {} (~{})", dep_name, version_range);

        // 检查缓存
        let cache_key = format!("{}:{}", dep_name, version_range);
        if let Some(cached_file) = cache.get(&cache_key) {
            println!("✅ 从缓存加载: {}", cached_file);
            continue;
        }

        // 获取版本列表并选择版本
        let versions = generate_sample_versions(dep_name);
        if let Some(selected_version) = select_version_for_range(&versions, version_range) {
            println!("选择版本: {}", selected_version.num);

            match test_download_version(dep_name, &selected_version.num, proxy_url) {
                Ok(success) => {
                    if success {
                        let filename = format!("{}-{}.crate", dep_name, selected_version.num);
                        cache.insert(cache_key, filename);
                        println!("✅ 下载并缓存成功");
                    } else {
                        println!("❌ 下载失败");
                    }
                }
                Err(e) => {
                    println!("❌ 下载出错: {}", e);
                }
            }
        } else {
            println!("❌ 未找到合适版本");
        }
    }

    println!("\n=== 缓存统计 ===");
    println!("缓存条目数: {}", cache.len());
    for (key, file) in cache.iter() {
        println!("  {} -> {}", key, file);
    }

    Ok(())
}