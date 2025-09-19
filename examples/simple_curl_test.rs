use curl::easy::{Easy};
use std::io::{self, Write};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 简单libcurl测试 ===");

    // 测试1: crates.io下载API（应该返回302重定向）
    let test_url = "https://crates.io/api/v1/crates/rand/1.0.0/download";
    println!("测试URL: {}", test_url);

    let mut handle = Easy::new();
    handle.url(test_url)?;
    handle.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
    handle.timeout(Duration::from_secs(30))?;

    // 不跟随重定向，看看原始响应
    handle.follow_location(false)?;
    handle.verbose(true)?;

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
        transfer.perform()?;
    }

    let response_code = handle.response_code()?;

    println!("\n=== 结果 ===");
    println!("HTTP状态码: {}", response_code);
    println!("响应头大小: {} 字节", headers.len());
    println!("响应体大小: {} 字节", body.len());

    // 显示响应头
    if !headers.is_empty() {
        println!("\n=== 响应头 ===");
        let header_str = String::from_utf8_lossy(&headers);
        println!("{}", header_str);
    }

    // 测试2: 跟随重定向
    println!("\n=== 测试跟随重定向 ===");
    let mut handle2 = Easy::new();
    handle2.url(test_url)?;
    handle2.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
    handle2.timeout(Duration::from_secs(30))?;
    handle2.follow_location(true)?;
    handle2.max_redirections(5)?;

    let mut final_body = Vec::new();
    {
        let mut transfer = handle2.transfer();
        transfer.write_function(|data| {
            final_body.extend_from_slice(data);
            Ok(data.len())
        })?;
        transfer.perform()?;
    }

    let final_response_code = handle2.response_code()?;
    let final_url = handle2.effective_url()?;

    println!("最终状态码: {}", final_response_code);
    println!("最终URL: {:?}", final_url);
    println!("最终文件大小: {} 字节", final_body.len());

    // 保存下载的文件
    if final_response_code == 200 && !final_body.is_empty() {
        std::fs::write("rand-1.0.0.crate", &final_body)?;
        println!("文件已保存为 rand-1.0.0.crate");

        // 验证文件是否是有效的tar.gz
        if final_body.starts_with(&[0x1f, 0x8b]) { // gzip magic number
            println!("✅ 文件是有效的gzip压缩格式");
        } else {
            println!("❌ 文件不是gzip格式");
        }
    }

    // 测试3: 测试不同的User-Agent
    println!("\n=== 测试不同的User-Agent ===");

    let user_agents = vec![
        "cargo 1.75.0 (1e801010e 2023-11-09)",
        "Mozilla/5.0 ( compatible crates-proxy/0.1.0 )",
        "curl/8.16.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    ];

    for ua in user_agents {
        println!("\n测试User-Agent: {}", ua);
        let mut handle3 = Easy::new();
        handle3.url(test_url)?;
        handle3.useragent(ua)?;
        handle3.timeout(Duration::from_secs(30))?;
        handle3.follow_location(true)?;
        handle3.max_redirections(5)?;

        let mut test_body = Vec::new();
        {
            let mut transfer = handle3.transfer();
            transfer.write_function(|data| {
                test_body.extend_from_slice(data);
                Ok(data.len())
            })?;
            transfer.perform()?;
        }

        let test_response_code = handle3.response_code()?;
        println!("状态码: {}, 大小: {} 字节", test_response_code, test_body.len());
    }

    // 测试4: 测试代理配置
    println!("\n=== 测试代理配置 ===");

    let proxy_configs = vec![
        ("HTTP代理", "http://172.16.0.80:9051"),
        ("SOCKS5代理", "socks5://172.16.0.80:9050"),
    ];

    for (proxy_name, proxy_url) in proxy_configs {
        println!("\n测试{}: {}", proxy_name, proxy_url);

        let mut handle4 = Easy::new();
        handle4.url(test_url)?;
        handle4.useragent("cargo 1.75.0 (1e801010e 2023-11-09)")?;
        handle4.timeout(Duration::from_secs(30))?;
        handle4.follow_location(true)?;
        handle4.max_redirections(5)?;

        // 设置代理
        match handle4.proxy(proxy_url) {
            Ok(_) => println!("代理设置成功"),
            Err(e) => {
                println!("代理设置失败: {}", e);
                continue;
            }
        }

        let mut proxy_body = Vec::new();
        {
            let mut transfer = handle4.transfer();
            transfer.write_function(|data| {
                proxy_body.extend_from_slice(data);
                Ok(data.len())
            })?;

            println!("通过代理执行请求...");
            match transfer.perform() {
                Ok(_) => println!("代理请求执行成功"),
                Err(e) => {
                    println!("代理请求执行失败: {}", e);
                    continue;
                }
            }
        }

        let proxy_response_code = handle4.response_code()?;
        let effective_url = handle4.effective_url()?;

        println!("状态码: {}", proxy_response_code);
        println!("最终URL: {:?}", effective_url);
        println!("大小: {} 字节", proxy_body.len());

        if proxy_response_code == 200 && !proxy_body.is_empty() {
            // 保存通过代理下载的文件
            let filename = format!("rand_via_{}.crate", proxy_name);
            std::fs::write(&filename, &proxy_body)?;
            println!("✅ 文件已保存为 {}", filename);

            // 验证文件格式
            if proxy_body.starts_with(&[0x1f, 0x8b]) {
                println!("✅ 文件是有效的gzip压缩格式");
            } else {
                println!("❌ 文件不是gzip格式");
            }
        } else if proxy_response_code == 403 {
            println!("⚠️  通过代理也返回403，可能是IP限制问题");
        }
    }

    Ok(())
}