#!/bin/bash

# crates_proxy 安装脚本
# 此脚本需要以root权限运行

set -e

# 变量定义
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="/usr/local/crates_proxy"
SERVICE_NAME="crates_proxy"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
CONFIG_FILE="${SCRIPT_DIR}/config.toml"
BINARY_FILE="${SCRIPT_DIR}/crates_proxy"

echo "开始安装 crates_proxy..."

# 检查是否以root权限运行
if [ "$(id -u)" -ne 0 ]; then
    echo "错误：此脚本需要以root权限运行"
    echo "请使用 sudo 运行此脚本"
    exit 1
fi

# 检查必要文件是否存在
if [ ! -f "${BINARY_FILE}" ]; then
    echo "错误：找不到 crates_proxy 可执行文件: ${BINARY_FILE}"
    exit 1
fi

if [ ! -f "${CONFIG_FILE}" ]; then
    echo "错误：找不到配置文件: ${CONFIG_FILE}"
    exit 1
fi

# 创建安装目录
echo "创建安装目录: ${INSTALL_DIR}"
mkdir -p "${INSTALL_DIR}"

# 创建缓存目录
CACHE_DIR="${INSTALL_DIR}/cache"
mkdir -p "${CACHE_DIR}"

# 复制可执行文件
echo "复制可执行文件..."
cp "${BINARY_FILE}" "${INSTALL_DIR}/"
chmod +x "${INSTALL_DIR}/crates_proxy"

# 复制配置文件
echo "复制配置文件..."
cp "${CONFIG_FILE}" "${INSTALL_DIR}/"

# 创建系统用户（如果不存在）
if ! id -u crates_proxy &>/dev/null; then
    echo "创建系统用户: crates_proxy"
    useradd -r -s /bin/false -d "${INSTALL_DIR}" crates_proxy
fi

# 设置文件权限
echo "设置文件权限..."
chown -R crates_proxy:crates_proxy "${INSTALL_DIR}"
chmod 640 "${INSTALL_DIR}/config.toml"

# 创建systemd服务文件
echo "创建systemd服务文件..."
cat > "${SERVICE_FILE}" << EOF
[Unit]
Description=Crates Proxy Server
After=network.target

[Service]
Type=simple
User=crates_proxy
Group=crates_proxy
WorkingDirectory=${INSTALL_DIR}
ExecStart=${INSTALL_DIR}/crates_proxy
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# 重新加载systemd
echo "重新加载systemd..."
systemctl daemon-reload

# 启用服务
echo "启用服务..."
systemctl enable "${SERVICE_NAME}"

# 启动服务
echo "启动服务..."
systemctl start "${SERVICE_NAME}"

# 检查服务状态
echo "检查服务状态..."
if systemctl is-active --quiet "${SERVICE_NAME}"; then
    echo "✓ crates_proxy 服务已成功启动"
else
    echo "✗ crates_proxy 服务启动失败，请检查日志: journalctl -u ${SERVICE_NAME}"
fi

echo "安装完成！"
echo ""
echo "服务管理命令："
echo "  启动服务: systemctl start ${SERVICE_NAME}"
echo "  停止服务: systemctl stop ${SERVICE_NAME}"
echo "  重启服务: systemctl restart ${SERVICE_NAME}"
echo "  查看状态: systemctl status ${SERVICE_NAME}"
echo "  查看日志: journalctl -u ${SERVICE_NAME} -f"
echo ""
echo "配置文件位置: ${INSTALL_DIR}/config.toml"
echo "可执行文件位置: ${INSTALL_DIR}/crates_proxy"