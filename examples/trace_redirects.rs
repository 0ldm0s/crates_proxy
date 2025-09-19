use curl::easy::{Easy};
use std::io::{self, Write};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 跟踪crates.io完整重定向链 ===");

    // 测试URL
    let test_url = "https://crates.io/api/v1/crates/rand/1.0.0/download";
    println!("初始URL: {}", test_url);

    // 第一步：不跟随重定向，看看第一个302
    println!("\n=== 第1步：crates.io API ===");
    let mut handle1 = Easy::new();
    handle1.url(test_url)?;
    handle1.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
    handle1.timeout(Duration::from_secs(30))?;
    handle1.follow_location(false)?; // 不跟随重定向
    handle1.verbose(true)?;

    let mut headers1 = Vec::new();
    {
        let mut transfer = handle1.transfer();
        transfer.header_function(|data| {
            headers1.extend_from_slice(data);
            true
        })?;
        transfer.perform()?;
    }

    let response_code1 = handle1.response_code()?;
    println!("状态码: {}", response_code1);

    // 解析重定向URL
    let headers_str1 = String::from_utf8_lossy(&headers1);
    let location_line = headers_str1.lines()
        .find(|line| line.to_lowercase().starts_with("location:"))
        .unwrap_or("");

    let binding1 = location_line.replace("location:", "");
    let redirect_url1 = binding1.trim();
    println!("重定向到: {}", redirect_url1);

    // 第二步：访问static.crates.io，看看是否还有重定向
    println!("\n=== 第2步：static.crates.io ===");
    let mut handle2 = Easy::new();
    handle2.url(redirect_url1)?;
    handle2.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
    handle2.timeout(Duration::from_secs(30))?;
    handle2.follow_location(false)?;
    handle2.verbose(true)?;

    let mut headers2 = Vec::new();
    let mut body2 = Vec::new();
    {
        let mut transfer = handle2.transfer();
        transfer.header_function(|data| {
            headers2.extend_from_slice(data);
            true
        })?;
        transfer.write_function(|data| {
            body2.extend_from_slice(data);
            Ok(data.len())
        })?;
        transfer.perform()?;
    }

    let response_code2 = handle2.response_code()?;
    println!("状态码: {}", response_code2);
    println!("响应体大小: {} 字节", body2.len());

    // 如果还有重定向，继续跟踪
    if response_code2 == 302 {
        let headers_str2 = String::from_utf8_lossy(&headers2);
        let location_line2 = headers_str2.lines()
            .find(|line| line.to_lowercase().starts_with("location:"))
            .unwrap_or("");

        let binding2 = location_line2.replace("location:", "");
        let redirect_url2 = binding2.trim();
        println!("再次重定向到: {}", redirect_url2);

        // 第三步：访问最终URL
        println!("\n=== 第3步：最终镜像服务器 ===");
        let mut handle3 = Easy::new();
        handle3.url(redirect_url2)?;
        handle3.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
        handle3.timeout(Duration::from_secs(30))?;
        handle3.follow_location(false)?;
        handle3.verbose(true)?;

        let mut headers3 = Vec::new();
        let mut body3 = Vec::new();
        {
            let mut transfer = handle3.transfer();
            transfer.header_function(|data| {
                headers3.extend_from_slice(data);
                true
            })?;
            transfer.write_function(|data| {
                body3.extend_from_slice(data);
                Ok(data.len())
            })?;
            transfer.perform()?;
        }

        let response_code3 = handle3.response_code()?;
        println!("最终状态码: {}", response_code3);
        println!("最终大小: {} 字节", body3.len());

        if response_code3 == 200 {
            println!("✅ 找到可用的镜像服务器!");
            println!("镜像服务器URL: {}", redirect_url2);

            // 保存测试文件
            std::fs::write("test_from_mirror.crate", &body3)?;
            println!("文件已保存为 test_from_mirror.crate");

            // 验证文件
            if body3.starts_with(&[0x1f, 0x8b]) {
                println!("✅ 文件是有效的gzip格式");
            }

            // 测试通过代理访问这个镜像服务器
            println!("\n=== 测试通过代理访问镜像服务器 ===");
            test_proxy_access(redirect_url2)?;
        }
    }

    Ok(())
}

fn test_proxy_access(final_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let proxy_configs = vec![
        ("HTTP代理", "http://172.16.0.80:9051"),
        ("SOCKS5代理", "socks5://172.16.0.80:9050"),
    ];

    for (proxy_name, proxy_url) in proxy_configs {
        println!("\n测试{}访问: {}", proxy_name, final_url);

        let mut handle = Easy::new();
        handle.url(final_url)?;
        handle.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
        handle.timeout(Duration::from_secs(30))?;
        handle.follow_location(false)?; // 镜像服务器应该直接返回文件

        match handle.proxy(proxy_url) {
            Ok(_) => println!("代理设置成功"),
            Err(e) => {
                println!("代理设置失败: {}", e);
                continue;
            }
        }

        let mut body = Vec::new();
        {
            let mut transfer = handle.transfer();
            transfer.write_function(|data| {
                body.extend_from_slice(data);
                Ok(data.len())
            })?;

            match transfer.perform() {
                Ok(_) => println!("请求成功"),
                Err(e) => {
                    println!("请求失败: {}", e);
                    continue;
                }
            }
        }

        let response_code = handle.response_code()?;
        println!("状态码: {}, 大小: {} 字节", response_code, body.len());

        if response_code == 200 && !body.is_empty() {
            let filename = format!("mirror_via_{}.crate", proxy_name);
            std::fs::write(&filename, &body)?;
            println!("✅ 通过{}下载成功，保存为 {}", proxy_name, filename);
        }
    }

    Ok(())
}