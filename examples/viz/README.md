# Intent-Lang 可视化示例

本目录展示了如何使用 intent-visualizer 工具可视化不同的 intent 文件。

## 在线查看

生成的可视化可以在支持Mermaid的平台上查看：
- GitHub（自动渲染）
- VS Code（安装Mermaid扩展）
- Notion
- GitLab

## 示例

### 1. 银行转账系统

**文件：** `examples/basics/transfer.intent`

**目标依赖图：**
```bash
intent-visualizer examples/basics/transfer.intent --type goal-graph
```

展示了"转账绝不能凭空创造或销毁资金"这个业务目标如何通过TransferSafe意图和TransferPreservesTotal定理实现。

### 2. 计费系统

**文件：** `examples/requirements/billing.intent`

**完备性矩阵：**
```bash
intent-visualizer examples/requirements/billing.intent --type coverage-matrix
```

展示了转账场景在 amount × account_state 两个维度上的9种组合覆盖情况。

**安全规则网络：**
```bash
intent-visualizer examples/requirements/billing.intent --type safety-network
```

展示了BalanceWithinOverdraft、OverdraftLimitNonNegative等安全规则如何约束Account类型。

### 3. 智能家居系统

**文件：** `examples/smarthome/smarthome.intent`

**意图关系图：**
```bash
intent-visualizer examples/smarthome/smarthome.intent --type intent-graph
```

展示了ArriveHome、GoodNight、LeaveHome、SetBrightness等意图之间通过Home和Light类型的数据流关系。

## 交互式HTML示例

### 生成单个文件的完整可视化

```bash
intent-visualizer examples/basics/transfer.intent \
  --interactive \
  -o examples/viz/transfer.html
```

在浏览器中打开 `transfer.html` 可以看到：
- 多个标签页切换不同可视化类型
- 彩色图形和交互式布局
- 源代码对照查看

### 生成所有可视化文件

```bash
# 为每个示例生成完整可视化套件
for file in examples/**/*.intent; do
  name=$(basename "$file" .intent)
  dir=$(dirname "$file")
  intent-visualizer "$file" --all --output-dir "$dir/viz-$name"
done
```

## 可视化说明

### 图例

| 符号 | 含义 |
|------|------|
| `[ ]` | Goal（业务目标） - 蓝色 |
| `{ }` | Safety（安全规则） - 橙色 |
| `(( ))` | Intent（意图声明） - 紫色 |
| `[[ ]]` | Theorem（定理） - 绿色 |
| `[/ /]` | Axiom（公理） - 粉色 |

### 箭头类型

| 箭头 | 含义 |
|------|------|
| `-->` | realizes（实现关系） |
| `-.->` | validates（验证关系） |
| `==>` | enforces（约束关系） |
| `---` | references（引用关系） |

### 标注说明

- `@tobe` - 待实现的意图（蓝色分组）
- `@asis` - 现状意图，可能有已知问题（黄色分组）
- `👥` - 利益相关方标记

## 将可视化嵌入文档

### 在Markdown中

直接复制Mermaid输出到Markdown文件：

```markdown
# 系统架构

下图展示了转账系统的业务目标实现路径：

\`\`\`mermaid
graph TD
  ...
\`\`\`
```

### 在HTML中

```html
<!DOCTYPE html>
<html>
<head>
  <script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
  <script>mermaid.initialize({startOnLoad:true});</script>
</head>
<body>
  <div class="mermaid">
    graph TD
      ...
  </div>
</body>
</html>
```

### 生成PNG/SVG图片

```bash
# 需要安装 mermaid-cli
npm install -g @mermaid-js/mermaid-cli

# 生成PNG
intent-visualizer transfer.intent --type goal-graph | \
  mmdc -i - -o transfer-goals.png

# 或直接生成SVG
intent-visualizer transfer.intent --format svg > transfer.svg
```

## 实际应用场景

### PRD评审

在产品需求评审时展示目标依赖图，让团队理解：
- 每个需求背后的业务目标
- 哪些安全规则保护这些目标
- 哪些定理证明了系统的正确性

### 技术文档

在架构文档中嵌入可视化：
- 系统概览 → Goal Graph
- 模块关系 → Intent Graph
- 安全设计 → Safety Network
- 测试策略 → Coverage Matrix

### 新人onboarding

为新成员生成交互式HTML，帮助快速理解：
- 系统的业务目标层次
- 各个模块的职责和关系
- 已有的安全保障措施

### 重构规划

使用Intent Graph识别高耦合模块，指导重构：
- 找出依赖过多的意图
- 识别循环依赖
- 规划解耦路径

## 下一步

- 查看完整用法：`intent-visualizer --help`
- 阅读详细指南：`tools/visualizer/GUIDE.md`
- 了解intent-lang语言：`docs/lang/README.md`
