#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
简单的crates代理测试脚本
"""

import requests
import time
import sys

def test_local_proxy():
    """测试本地代理服务器"""
    print("测试本地代理服务器...")

    # 测试下载rand 1.0.0版本
    url = "http://127.0.0.1:8080/api/v1/crates/rand/1.0.0/download"

    headers = {
        'User-Agent': 'cargo 1.75.0 (1e801010e 2023-11-09)'
    }

    try:
        print(f"请求: {url}")
        response = requests.get(url, headers=headers, timeout=30)

        print(f"状态码: {response.status_code}")
        print(f"响应头: {dict(response.headers)}")

        if response.status_code == 200:
            print(f"✅ 下载成功! 文件大小: {len(response.content)} 字节")

            # 保存文件
            with open("test_rand.tar.gz", "wb") as f:
                f.write(response.content)
            print("文件已保存为 test_rand.tar.gz")
            return True
        else:
            print(f"❌ 下载失败: {response.text}")
            return False

    except Exception as e:
        print(f"❌ 请求异常: {e}")
        return False

def test_direct_download():
    """测试直接下载"""
    print("\n测试直接下载...")

    url = "https://crates.io/api/v1/crates/rand/1.0.0/download"

    headers = {
        'User-Agent': 'cargo 1.75.0 (1e801010e 2023-11-09)'
    }

    try:
        print(f"请求: {url}")
        response = requests.get(url, headers=headers, timeout=30)

        print(f"状态码: {response.status_code}")

        if response.status_code == 200:
            print(f"✅ 下载成功! 文件大小: {len(response.content)} 字节")
            return True
        else:
            print(f"❌ 下载失败: {response.text}")
            return False

    except Exception as e:
        print(f"❌ 请求异常: {e}")
        return False

if __name__ == "__main__":
    print("=== 简单代理测试 ===")

    # 测试本地代理
    local_success = test_local_proxy()

    # 测试直接下载
    direct_success = test_direct_download()

    print(f"\n=== 结果 ===")
    print(f"本地代理: {'✅ 成功' if local_success else '❌ 失败'}")
    print(f"直接下载: {'✅ 成功' if direct_success else '❌ 失败'}")