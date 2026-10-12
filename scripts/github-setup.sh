#!/usr/bin/env bash
# CloudMount · GitHub 首次推送脚本
#
# 前提（二选一，完成后重新运行本脚本）：
#   A) 浏览器登录 GitHub → Settings → SSH and GPG keys → New SSH key，
#      粘贴 tools/../.ssh 公钥内容（脚本会打印）；
#   B) 或运行: tools/bin/gh.exe auth login   （按提示走浏览器/粘贴 token）
#
# 用法:
#   bash scripts/github-setup.sh <你的GitHub用户名> [仓库名=cloudmount]

set -euo pipefail
cd "$(dirname "$0")/.."

GH=tools/bin/gh.exe
USER_NAME="${1:?用法: bash scripts/github-setup.sh <GitHub用户名> [仓库名]}"
REPO="${2:-cloudmount}"

echo "==> 1/4 检查认证状态"
if "$GH" auth status >/dev/null 2>&1; then
    echo "    gh 已认证: $("$GH" api user --jq .login)"
else
    echo "    gh 未认证，测试 SSH..."
    if ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 -T git@github.com 2>&1 | grep -q "successfully authenticated"; then
        echo "    SSH 认证可用"
        REMOTE="git@github.com:${USER_NAME}/${REPO}.git"
    else
        echo "!!  未认证。请先完成以下任一操作后重试："
        echo "    A) 把下面的 SSH 公钥添加到 https://github.com/settings/keys ："
        cat ~/.ssh/id_ed25519.pub
        echo "    B) 或运行: $GH auth login"
        exit 1
    fi
fi
REMOTE="${REMOTE:-https://github.com/${USER_NAME}/${REPO}.git}"

echo "==> 2/4 创建远端仓库 ${USER_NAME}/${REPO}（公开）"
if "$GH" repo view "${USER_NAME}/${REPO}" >/dev/null 2>&1; then
    echo "    仓库已存在，跳过创建"
else
    "$GH" repo create "${USER_NAME}/${REPO}" --public --description \
        "把 115 网盘变成所有设备上的同步盘：OneDrive 式体验 · Rust 极致轻量 · 全开源" \
        --homepage "https://github.com/${USER_NAME}/${REPO}#readme"
fi

echo "==> 3/4 配置 remote 并推送"
if git remote get-url origin >/dev/null 2>&1; then
    git remote set-url origin "$REMOTE"
else
    git remote add origin "$REMOTE"
fi
git push -u origin main

echo "==> 4/4 完成"
echo "    仓库: https://github.com/${USER_NAME}/${REPO}"
echo "    CI 会在 GitHub Actions 上跑三平台检查（.github/workflows/ci.yml）"
