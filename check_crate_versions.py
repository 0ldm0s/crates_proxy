#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
比较crates.io和USTC镜像的crate版本差异
"""

import requests
import json
import sys
from packaging import version
import argparse

def get_crates_io_version(crate_name):
    """从crates.io获取最新版本"""
    url = f"https://crates.io/api/v1/crates/{crate_name}"
    headers = {
        'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    }
    try:
        response = requests.get(url, headers=headers, timeout=10)
        response.raise_for_status()
        data = response.json()
        latest_version = data['crate']['max_version']
        return latest_version
    except requests.RequestException as e:
        print(f"获取crates.io版本失败: {e}")
        return None

def get_ustc_version(crate_name):
    """从USTC镜像获取最新版本"""
    url = f"https://mirrors.ustc.edu.cn/crates.io-index/{crate_name[0:2]}/{crate_name[2:4]}/{crate_name}"
    headers = {
        'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    }
    try:
        response = requests.get(url, headers=headers, timeout=10)
        response.raise_for_status()

        # USTC镜像返回的是文本格式，每行一个版本信息
        lines = response.text.strip().split('\n')
        versions = []

        for line in lines:
            if line.strip():
                try:
                    data = json.loads(line)
                    if 'vers' in data:
                        versions.append(data['vers'])
                except json.JSONDecodeError:
                    continue

        if versions:
            # 找到最新的版本
            latest_version = max(versions, key=lambda v: version.parse(v))
            return latest_version
        else:
            return None

    except requests.RequestException as e:
        print(f"获取USTC镜像版本失败: {e}")
        return None

def compare_versions(crate_name):
    """比较两个源的版本"""
    print(f"检查crate: {crate_name}")
    print("-" * 50)

    crates_io_version = get_crates_io_version(crate_name)
    ustc_version = get_ustc_version(crate_name)

    if crates_io_version is None or ustc_version is None:
        print("获取版本信息失败")
        return False

    print(f"crates.io 版本: {crates_io_version}")
    print(f"USTC镜像版本: {ustc_version}")

    # 比较版本
    crates_io_ver = version.parse(crates_io_version)
    ustc_ver = version.parse(ustc_version)

    if crates_io_ver > ustc_ver:
        print("✅ crates.io版本较新")
        print(f"版本差异: {ustc_version} -> {crates_io_version}")
        return True
    elif crates_io_ver < ustc_ver:
        print("❓ USTC镜像版本较新 (这不太正常)")
        return True
    else:
        print("✅ 版本一致")
        return False

def main():
    parser = argparse.ArgumentParser(description='比较crates.io和USTC镜像的crate版本')
    parser.add_argument('crate', help='要检查的crate名称')
    parser.add_argument('--all', action='store_true', help='检查当前项目的所有依赖')

    args = parser.parse_args()

    if args.all:
        # 这里可以扩展为读取Cargo.toml检查所有依赖
        print("检查所有依赖功能尚未实现")
        return

    has_diff = compare_versions(args.crate)

    if has_diff:
        sys.exit(1)
    else:
        sys.exit(0)

if __name__ == "__main__":
    main()