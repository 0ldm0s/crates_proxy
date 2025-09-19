#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
验证crates代理服务器功能的脚本
测试HTTP和SOCKS5代理配置下的下载功能
"""

import requests
import json
import time
import os
import sys
import subprocess
import argparse
from pathlib import Path

def test_proxy_download(crate_name, proxy_url=None, use_local_proxy=False):
    """测试通过代理下载crate"""
    print(f"测试下载crate: {crate_name}")
    print("-" * 50)

    # 构建请求URL - 测试下载请求
    if use_local_proxy:
        # 使用本地代理服务器 - 必须包含完整路径
        url = f"http://127.0.0.1:8080/api/v1/crates/{crate_name}/1.0.0/download"
        proxies = None  # 不设置代理，直接访问本地代理服务器
        print(f"通过本地代理服务器: {url}")
    else:
        # 先获取crate信息找到下载链接
        info_url = f"https://crates.io/api/v1/crates/{crate_name}"
        if proxy_url:
            proxies = {
                'http': proxy_url,
                'https': proxy_url
            }
            print(f"通过代理 {proxy_url} 获取信息: {info_url}")
        else:
            proxies = None
            print(f"直接获取信息: {info_url}")

        # 获取crate信息找到下载链接
        try:
            response = requests.get(info_url, headers=headers, proxies=proxies, timeout=10)
            if response.status_code == 200:
                data = response.json()
                # 获取最新版本的下载链接
                latest_version = data['crate']['max_version']
                url = f"https://crates.io/api/v1/crates/{crate_name}/{latest_version}/download"
                print(f"下载链接: {url}")
            else:
                print(f"❌ 获取crate信息失败: {response.status_code}")
                return False, 0, 0
        except Exception as e:
            print(f"❌ 获取crate信息异常: {e}")
            return False, 0, 0

    headers = {
        'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    }

    try:
        start_time = time.time()
        response = requests.get(url, headers=headers, proxies=proxies, timeout=30)
        end_time = time.time()

        if response.status_code == 200:
            file_size = len(response.content)
            download_time = end_time - start_time

            print(f"✅ 下载成功")
            print(f"   状态码: {response.status_code}")
            print(f"   文件大小: {file_size} 字节")
            print(f"   下载时间: {download_time:.2f} 秒")

            # 保存测试文件
            test_file = f"test_{crate_name}.tar.gz"
            with open(test_file, 'wb') as f:
                f.write(response.content)
            print(f"   保存到: {test_file}")

            return True, download_time, file_size
        else:
            print(f"❌ 下载失败")
            print(f"   状态码: {response.status_code}")
            print(f"   响应: {response.text[:200]}")
            return False, 0, 0

    except Exception as e:
        print(f"❌ 下载异常: {e}")
        return False, 0, 0

def test_crate_info(crate_name, use_local_proxy=False):
    """测试获取crate信息"""
    print(f"测试获取crate信息: {crate_name}")
    print("-" * 30)

    if use_local_proxy:
        url = f"http://127.0.0.1:8080/api/v1/crates/{crate_name}"
        print(f"通过本地代理服务器: {url}")
    else:
        url = f"https://crates.io/api/v1/crates/{crate_name}"
        print(f"直接获取: {url}")

    headers = {
        'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    }

    try:
        response = requests.get(url, headers=headers, timeout=10)
        if response.status_code == 200:
            data = response.json()
            latest_version = data['crate']['max_version']
            print(f"✅ 获取成功")
            print(f"   最新版本: {latest_version}")
            print(f"   描述: {data['crate']['description'][:100]}...")
            return True, latest_version
        else:
            print(f"❌ 获取失败: {response.status_code}")
            return False, None
    except Exception as e:
        print(f"❌ 获取异常: {e}")
        return False, None

def check_cache_exists(crate_name, version="latest"):
    """检查缓存是否存在"""
    cache_path = Path("./cache")
    if not cache_path.exists():
        print(f"❌ 缓存目录不存在: {cache_path}")
        return False

    # 查找对应的缓存文件
    crate_cache_path = cache_path / crate_name
    if not crate_cache_path.exists():
        print(f"❌ crate缓存目录不存在: {crate_cache_path}")
        return False

    print(f"✅ 缓存目录存在: {crate_cache_path}")

    # 列出缓存文件
    try:
        files = list(crate_cache_path.rglob("*"))
        files = [f for f in files if f.is_file()]
        print(f"   缓存文件数量: {len(files)}")
        for f in files[:5]:  # 只显示前5个文件
            print(f"   - {f.relative_to(cache_path)}")
        if len(files) > 5:
            print(f"   ... 还有 {len(files) - 5} 个文件")
        return len(files) > 0
    except Exception as e:
        print(f"❌ 读取缓存失败: {e}")
        return False

def configure_cargo_proxy():
    """配置cargo使用本地代理"""
    cargo_config = Path.home() / ".cargo" / "config.toml"

    config_content = """[source.crates-io]
replace-with = "local-proxy"

[source.local-proxy]
registry = "http://127.0.0.1:8080"

[net]
git-fetch-with-cli = true
"""

    try:
        cargo_config.parent.mkdir(exist_ok=True)
        with open(cargo_config, 'w') as f:
            f.write(config_content)
        print(f"✅ cargo配置已更新: {cargo_config}")
        return True
    except Exception as e:
        print(f"❌ 配置cargo失败: {e}")
        return False

def test_cargo_build():
    """测试cargo编译"""
    print("测试cargo编译...")
    print("-" * 30)

    # 创建测试项目
    test_dir = Path("./test_crate")
    if test_dir.exists():
        subprocess.run(["rm", "-rf", str(test_dir)], check=True)

    try:
        # 创建新项目
        subprocess.run(["cargo", "new", str(test_dir)], check=True, capture_output=True)

        # 添加依赖
        cargo_toml = test_dir / "Cargo.toml"
        with open(cargo_toml, 'a') as f:
            f.write('\n[dependencies]\nrand = "1.0"\nserde = "1.0"\n')

        # 编译项目
        result = subprocess.run(
            ["cargo", "build", "--manifest-path", str(cargo_toml)],
            capture_output=True,
            text=True,
            timeout=60
        )

        if result.returncode == 0:
            print("✅ cargo编译成功")
            return True
        else:
            print("❌ cargo编译失败")
            print(f"错误: {result.stderr[:500]}")
            return False

    except subprocess.TimeoutExpired:
        print("❌ cargo编译超时")
        return False
    except Exception as e:
        print(f"❌ cargo编译异常: {e}")
        return False

def main():
    parser = argparse.ArgumentParser(description='验证crates代理服务器功能')
    parser.add_argument('--crate', default='rand', help='要测试的crate名称 (默认: rand)')
    parser.add_argument('--http-proxy', default='http://172.16.0.80:9051', help='HTTP代理地址')
    parser.add_argument('--socks5-proxy', default='socks5://172.16.0.80:9050', help='SOCKS5代理地址')
    parser.add_argument('--configure-cargo', action='store_true', help='配置cargo使用本地代理')
    parser.add_argument('--test-cargo', action='store_true', help='测试cargo编译')

    args = parser.parse_args()

    print("=== Crates代理验证脚本 ===")
    print(f"测试crate: {args.crate}")
    print()

    # 配置cargo
    if args.configure_cargo:
        configure_cargo_proxy()

    # 测试直接下载
    print("1. 测试直接下载...")
    success, time1, size1 = test_proxy_download(args.crate)

    if success:
        print(f"\n2. 测试本地代理服务器...")
        success2, time2, size2 = test_proxy_download(args.crate, use_local_proxy=True)

        if success2:
            # 检查缓存
            print(f"\n3. 检查缓存...")
            check_cache_exists(args.crate)

            # 测试缓存命中
            print(f"\n4. 测试缓存命中...")
            success3, time3, size3 = test_proxy_download(args.crate, use_local_proxy=True)

            if success3 and time3 < time2:
                print(f"✅ 缓存生效! 第二次下载更快 ({time3:.2f}s vs {time2:.2f}s)")
            elif success3:
                print(f"⚠️  两次下载时间相近 ({time3:.2f}s vs {time2:.2f}s)")

        # 测试crate信息
        print(f"\n5. 测试获取crate信息...")
        test_crate_info(args.crate, use_local_proxy=True)

    # 测试cargo编译
    if args.test_cargo:
        print(f"\n6. 测试cargo编译...")
        test_cargo_build()

    print("\n=== 验证完成 ===")

if __name__ == "__main__":
    main()