#!/bin/bash

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Crates Proxy 多架构 Deb 包构建脚本 ===${NC}"

# 检查是否安装了必要的依赖
echo -e "${YELLOW}检查依赖...${NC}"
if ! dpkg -l | grep -q "debhelper"; then
    echo -e "${RED}错误: 请先安装 debhelper${NC}"
    echo "运行: sudo apt update && sudo apt install -y debhelper dh-rust cargo build-essential crossbuild-essential-amd64 gcc-x86-64-linux-gnu"
    exit 1
fi

# 清理之前的构建
echo -e "${YELLOW}清理之前的构建...${NC}"
rm -rf debian/.debhelper debian/crates-proxy debian/files debian/*.substvars debian/*.debhelper.log
rm -rf target/release
rm -rf ../*.deb ../*.changes ../*.buildinfo

# 设置版本号
VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
echo -e "${GREEN}构建版本: ${VERSION}${NC}"

# 构建ARM64包（本机架构）
echo -e "${GREEN}构建ARM64包...${NC}"
dpkg-buildpackage -us -uc -b --host-arch arm64
echo -e "${GREEN}ARM64包构建完成${NC}"

# 清理
echo -e "${YELLOW}清理...${NC}"
rm -rf debian/.debhelper debian/crates-proxy debian/files debian/*.substvars debian/*.debhelper.log
rm -rf target/release

# 构建AMD64包（交叉编译）
echo -e "${GREEN}构建AMD64包...${NC}"
dpkg-buildpackage -us -uc -b --host-arch amd64
echo -e "${GREEN}AMD64包构建完成${NC}"

# 显示结果
echo -e "${GREEN}=== 构建完成 ===${NC}"
echo -e "${YELLOW}生成的包文件:${NC}"
ls -la ../*.deb | while read -r line; do
    filename=$(echo "$line" | awk '{print $9}')
    size=$(echo "$line" | awk '{print $5}')
    arch=$(echo "$filename" | sed -n 's/.*_\([^_]*\)\.deb$/\1/p')
    echo -e "  ${GREEN}$filename${NC} (${arch}) - ${size} bytes"
done

echo ""
echo -e "${YELLOW}安装说明:${NC}"
echo "  ARM64: sudo dpkg -i ../crates-proxy_${VERSION}-1_arm64.deb"
echo "  AMD64: sudo dpkg -i ../crates-proxy_${VERSION}-1_amd64.deb"

echo ""
echo -e "${YELLOW}测试安装:${NC}"
echo "  systemctl status crates-proxy"
echo "  journalctl -u crates-proxy -f"