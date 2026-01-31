#!/bin/bash

# AgentX macOS 安装脚本
# 自动将应用复制到 Applications 并移除 Gatekeeper 隔离属性

set -e

echo "🚀 AgentX 安装脚本"
echo "=================="
echo ""

# 检测是否在 DMG 中运行
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_PATH="$SCRIPT_DIR/AgentX.app"

if [ ! -d "$APP_PATH" ]; then
    echo "❌ 错误：找不到 AgentX.app"
    echo "   请确保此脚本在 DMG 挂载目录中运行。"
    exit 1
fi

echo "📦 找到应用：$APP_PATH"
echo ""

# 检查 Applications 目录
if [ ! -d "/Applications" ]; then
    echo "❌ 错误：找不到 /Applications 目录"
    exit 1
fi

# 检查是否已安装
if [ -d "/Applications/AgentX.app" ]; then
    echo "⚠️  检测到已安装的版本"
    read -p "是否覆盖安装？(y/N) " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "❌ 安装已取消"
        exit 0
    fi
    echo "🗑️  正在删除旧版本..."
    rm -rf "/Applications/AgentX.app"
fi

# 复制应用到 Applications
echo "📋 正在复制应用到 Applications..."
cp -R "$APP_PATH" /Applications/

# 移除隔离属性
echo "🔓 正在移除 Gatekeeper 隔离属性..."
xattr -cr /Applications/AgentX.app

# 验证
if [ -d "/Applications/AgentX.app" ]; then
    echo ""
    echo "✅ 安装成功！"
    echo ""
    echo "AgentX 已安装到 /Applications/AgentX.app"
    echo ""

    # 询问是否立即启动
    read -p "是否立即启动 AgentX？(Y/n) " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        echo "🚀 正在启动 AgentX..."
        open /Applications/AgentX.app
    else
        echo "💡 你可以随时从 Applications 文件夹启动 AgentX"
    fi
else
    echo ""
    echo "❌ 安装失败"
    echo "   请手动将 AgentX.app 拖到 Applications 文件夹"
    exit 1
fi

echo ""
echo "🎉 安装完成！"
