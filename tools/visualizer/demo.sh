#!/bin/bash
# Intent-Lang 可视化工具演示脚本

set -e

echo "🎯 Intent-Lang Visualization Demo"
echo "=================================="
echo

# 确保工具已构建
if [ ! -f "target/debug/intent-visualizer" ]; then
    echo "📦 Building visualizer..."
    cargo build -p intent-visualizer
    echo
fi

VISUALIZER="./target/debug/intent-visualizer"

# 创建输出目录
mkdir -p examples/viz-demo

echo "1️⃣  生成转账系统的目标依赖图..."
$VISUALIZER examples/basics/transfer.intent \
    --type goal-graph \
    -o examples/viz-demo/transfer-goals.mmd
echo "   ✅ 已保存到: examples/viz-demo/transfer-goals.mmd"
echo

echo "2️⃣  生成智能家居的意图关系图..."
$VISUALIZER examples/smarthome/smarthome.intent \
    --type intent-graph \
    -o examples/viz-demo/smarthome-intents.mmd
echo "   ✅ 已保存到: examples/viz-demo/smarthome-intents.mmd"
echo

echo "3️⃣  生成计费系统的完备性矩阵..."
$VISUALIZER examples/requirements/billing.intent \
    --type coverage-matrix \
    -o examples/viz-demo/billing-coverage.mmd
echo "   ✅ 已保存到: examples/viz-demo/billing-coverage.mmd"
echo

echo "4️⃣  生成交互式HTML（转账系统）..."
$VISUALIZER examples/basics/transfer.intent \
    --interactive \
    -o examples/viz-demo/transfer-interactive.html
echo "   ✅ 已保存到: examples/viz-demo/transfer-interactive.html"
echo "   💡 在浏览器中打开查看: open examples/viz-demo/transfer-interactive.html"
echo

echo "5️⃣  生成完整可视化套件（计费系统）..."
$VISUALIZER examples/requirements/billing.intent \
    --all \
    --output-dir examples/viz-demo/billing-all
echo "   ✅ 已保存到: examples/viz-demo/billing-all/"
echo

echo "🎉 演示完成！"
echo
echo "生成的文件："
echo "  - examples/viz-demo/transfer-goals.mmd"
echo "  - examples/viz-demo/smarthome-intents.mmd"
echo "  - examples/viz-demo/billing-coverage.mmd"
echo "  - examples/viz-demo/transfer-interactive.html (在浏览器中打开)"
echo "  - examples/viz-demo/billing-all/ (完整套件)"
echo
echo "查看更多示例："
echo "  intent-visualizer --help"
echo "  cat tools/visualizer/GUIDE.md"
