# Intent-Lang Visualization Demo Gallery

这个目录包含了 intent-lang 可视化工具生成的交互式演示文件。

## 🏠 主索引页面

**打开方式：**
```bash
open examples/viz-demo/index.html
```

主索引提供了一个美观的画廊视图，包含：

### 可视化项目

1. **💰 Transfer System（转账系统）**
   - 文件：`transfer-interactive.html`
   - 包含：目标依赖图、意图流程、验证流程、源代码
   - 特点：展示如何检测有bug的意图

2. **💳 Billing System（计费系统）**
   - 目录：`billing-all/`
   - 文件：`billing-all/index.html`
   - 包含：完整的可视化套件（4种类型）
   - 特点：完整的业务领域建模示例

3. **🏠 Smart Home（智能家居）**
   - 文件：`smarthome-intents.mmd`
   - 包含：意图关系图
   - 特点：展示语音控制意图的数据流

### 文档资源

主索引还提供了快速访问以下文档的链接：
- 工具文档
- 使用指南
- 项目总结
- Intent-Lang语言规范
- 可视化示例说明
- 故障排除

## 📁 目录结构

```
viz-demo/
├── index.html                      # 🌟 主索引（从这里开始）
│
├── transfer-interactive.html       # 转账系统交互式可视化
├── transfer-goals.mmd             # 转账系统目标依赖图（Mermaid）
│
├── billing-all/                    # 计费系统完整套件
│   ├── index.html                 # 📊 计费系统索引（交互式）
│   ├── goalgraph.mmd              # 目标依赖图
│   ├── intentgraph.mmd            # 意图关系图
│   ├── safetynetwork.mmd          # 安全规则网络
│   └── coveragematrix.mmd         # 完备性矩阵
│
├── smarthome-intents.mmd          # 智能家居意图关系图
└── billing-coverage.mmd           # 计费系统完备性矩阵
```

## 🎯 使用方式

### 方式1：从主索引开始（推荐）

```bash
open examples/viz-demo/index.html
```

在主页面中：
- 点击任意卡片查看对应的可视化
- 卡片会高亮显示可用的可视化类型
- 点击"View Interactive"或"View Suite"按钮进入

### 方式2：直接打开特定可视化

```bash
# 查看转账系统
open examples/viz-demo/transfer-interactive.html

# 查看计费系统套件
open examples/viz-demo/billing-all/index.html
```

### 方式3：在Markdown中嵌入

.mmd文件可以直接嵌入到Markdown文档中：

```markdown
# 系统架构

\`\`\`mermaid
[复制 .mmd 文件内容]
\`\`\`
```

## 🔄 重新生成

如果需要重新生成所有可视化：

```bash
# 删除旧文件
rm -rf examples/viz-demo

# 运行演示脚本
./tools/visualizer/demo.sh

# 主索引会自动创建
```

或者手动生成特定项目：

```bash
# 生成转账系统交互式HTML
intent-lang-visualizer examples/basics/transfer.intent \
  --interactive -o examples/viz-demo/transfer-interactive.html

# 生成计费系统完整套件
intent-lang-visualizer examples/requirements/billing.intent \
  --all --output-dir examples/viz-demo/billing-all
```

## 🎨 索引页面特点

### 主索引 (index.html)
- ✅ 画廊式卡片布局
- ✅ 渐变色设计
- ✅ 悬停动画效果
- ✅ 可点击卡片导航
- ✅ 文档资源快速访问
- ✅ 响应式设计（移动端友好）

### 子系统索引 (billing-all/index.html)
- ✅ 标签页切换不同可视化类型
- ✅ 实时渲染Mermaid图表
- ✅ 下载原始.mmd文件链接
- ✅ 每个可视化都有说明文字
- ✅ 统一的视觉风格

## 📊 可视化类型说明

### Goal Graph（目标依赖图）
展示业务目标如何通过安全规则、意图和定理实现。

**用途：**
- 需求追溯
- PRD评审
- 识别未实现的目标

### Intent Graph（意图关系图）
展示意图之间的数据流和依赖关系，按@tobe/@asis分组。

**用途：**
- 识别模块耦合
- 规划重构
- 发现循环依赖

### Safety Network（安全规则网络）
展示安全规则覆盖的类型和约束维度。

**用途：**
- 安全审计
- Gap分析
- 理解类型约束

### Coverage Matrix（完备性矩阵）
可视化多维度测试场景的完备性。

**用途：**
- 评估测试覆盖率
- 识别未覆盖场景
- 指导测试用例设计

## 🔍 浏览器兼容性

所有HTML文件使用标准Web技术：
- ✅ Chrome/Edge (推荐)
- ✅ Firefox
- ✅ Safari
- ⚠️ 需要启用JavaScript
- ⚠️ 需要网络连接（加载Mermaid.js CDN）

离线使用：可以下载Mermaid.js到本地并修改HTML中的script标签。

## 🐛 故障排除

### 问题：图表不显示

**解决方案：**
1. 检查浏览器控制台是否有错误
2. 确认网络连接正常（需要加载CDN）
3. 尝试刷新页面（Ctrl+R / Cmd+R）

### 问题：点击卡片没有反应

**解决方案：**
1. 检查目标文件是否存在
2. 使用相对路径确认文件位置
3. 查看浏览器控制台错误信息

### 问题：.mmd文件在浏览器中无法渲染

**说明：**
.mmd文件是Markdown格式，浏览器无法直接渲染。

**解决方案：**
- 在支持Mermaid的Markdown查看器中打开（VS Code + Mermaid扩展）
- 复制内容到在线Mermaid编辑器：https://mermaid.live/
- 使用index.html查看渲染后的版本

## 💡 最佳实践

1. **从主索引开始** - 提供了最佳的导航体验
2. **使用交互式HTML** - 比静态.mmd文件更直观
3. **保存为书签** - 将常用的可视化页面加入浏览器书签
4. **定期重新生成** - 保持可视化与代码同步
5. **分享给团队** - 将HTML文件部署到内部服务器或GitHub Pages

## 🚀 高级用法

### 部署到GitHub Pages

```bash
# 1. 生成所有可视化
./tools/visualizer/demo.sh

# 2. 提交到git
git add examples/viz-demo/
git commit -m "Update visualizations"
git push

# 3. 启用GitHub Pages
# Settings → Pages → Source: main branch, /examples/viz-demo
```

访问：`https://yourusername.github.io/intent-lang/examples/viz-demo/`

### 嵌入到文档站点

可以将生成的HTML嵌入到Docusaurus、VuePress等文档站点中。

## 📚 相关资源

- [可视化工具文档](../../tools/visualizer/README.md)
- [详细使用指南](../../tools/visualizer/GUIDE.md)
- [技术实现总结](../../tools/visualizer/SUMMARY.md)
- [Intent-Lang语言规范](../../docs/lang/README.md)

---

**生成工具版本：** intent-lang-visualizer v0.1.0  
**最后更新：** 2026-06-15
