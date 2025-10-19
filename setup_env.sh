#!/bin/bash
# LLVM环境变量设置脚本
# 使用方法: source setup_env.sh

export LLVM_SYS_140_PREFIX=/usr/lib/llvm-14

echo "LLVM环境变量已设置:"
echo "LLVM_SYS_140_PREFIX=$LLVM_SYS_140_PREFIX"
echo ""
echo "现在可以运行 cargo build 或 cargo run"

