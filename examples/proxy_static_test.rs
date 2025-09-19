use curl::easy::{Easy};
use std::io::{self, Write};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 测试每步都加上代理的curl请求 ===");

    // 已知的static.crates.io地址
    let static_url = "https://static.crates.io/crates/rand/rand-1.0.0.crate";
    println!("测试URL: {}", static_url);

    // 代理配置
    let proxy_configs = vec![
        ("HTTP代理", "http://172.16.0.80:9051"),
        ("SOCKS5代理", "socks5://172.16.0.80:9050"),
    ];

    for (proxy_name, proxy_url) in proxy_configs {
        println!("\n=== 测试{}访问 {} ===", proxy_name, static_url);

        let mut handle = Easy::new();
        handle.url(static_url)?;
        handle.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
        handle.timeout(Duration::from_secs(30))?;
        handle.follow_location(false)?; // 不跟随重定向，直接测试static.crates.io
        handle.verbose(true)?;

        // 设置代理
        match handle.proxy(proxy_url) {
            Ok(_) => println!("✅ 代理设置成功: {}", proxy_url),
            Err(e) => {
                println!("❌ 代理设置失败: {}", e);
                continue;
            }
        }

        let mut headers = Vec::new();
        let mut body = Vec::new();

        {
            let mut transfer = handle.transfer();

            transfer.header_function(|data| {
                headers.extend_from_slice(data);
                true
            })?;

            transfer.write_function(|data| {
                body.extend_from_slice(data);
                Ok(data.len())
            })?;

            println!("执行请求...");
            match transfer.perform() {
                Ok(_) => println!("✅ 请求执行成功"),
                Err(e) => {
                    println!("❌ 请求执行失败: {}", e);
                    continue;
                }
            }
        }

        let response_code = handle.response_code()?;
        println!("状态码: {}", response_code);
        println!("响应头大小: {} 字节", headers.len());
        println!("响应体大小: {} 字节", body.len());

        // 显示响应头
        if !headers.is_empty() {
            println!("\n=== 响应头 ===");
            let header_str = String::from_utf8_lossy(&headers);
            for line in header_str.lines() {
                if !line.trim().is_empty() {
                    println!("  {}", line);
                }
            }
        }

        // 显示响应体（如果是错误信息）
        if response_code != 200 && !body.is_empty() {
            println!("\n=== 响应体 ===");
            let body_str = String::from_utf8_lossy(&body);
            println!("  {}", body_str);
        }

        // 如果成功，保存文件
        if response_code == 200 && !body.is_empty() {
            let filename = format!("rand_via_{}.crate", proxy_name);
            std::fs::write(&filename, &body)?;
            println!("✅ 文件已保存为: {}", filename);

            // 验证文件格式
            if body.starts_with(&[0x1f, 0x8b]) {
                println!("✅ 文件是有效的gzip压缩格式");
            } else {
                println!("❌ 文件不是gzip格式");
            }
        } else if response_code == 403 {
            println!("❌ 返回403禁止访问，可能是AWS S3的访问限制");
        }
    }

    // 额外测试：尝试不通过代理直接访问
    println!("\n=== 对比测试：不使用代理直接访问 ===");

    let mut handle = Easy::new();
    handle.url(static_url)?;
    handle.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
    handle.timeout(Duration::from_secs(30))?;
    handle.follow_location(false)?;
    handle.verbose(true)?;

    let mut body = Vec::new();
    {
        let mut transfer = handle.transfer();
        transfer.write_function(|data| {
            body.extend_from_slice(data);
            Ok(data.len())
        })?;
        transfer.perform()?;
    }

    let response_code = handle.response_code()?;
    println!("直接访问状态码: {}, 大小: {} 字节", response_code, body.len());

    Ok(())
}