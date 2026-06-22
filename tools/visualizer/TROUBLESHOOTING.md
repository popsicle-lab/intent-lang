# Mermaid 可视化故障排除指南

## 问题：图表不显示

如果在浏览器中打开HTML文件后，部分或全部Mermaid图表没有显示，请按照以下步骤诊断：

### 步骤1：检查浏览器控制台

1. 在浏览器中打开问题页面
2. 按 `F12` (Windows/Linux) 或 `Cmd+Option+I` (Mac) 打开开发者工具
3. 切换到 **Console** 标签
4. 查找红色错误信息

#### 常见错误信息及解决方案

**错误1: "Diagram error"**
```
Error: Parse error on line X: ...
```
**原因：** Mermaid语法错误  
**解决：** 检查图表语法，确保符合Mermaid规范

**错误2: "Cannot read property 'xxx' of undefined"**  
**原因：** Mermaid.js未正确加载  
**解决：** 检查网络连接，或使用本地Mermaid.js

**错误3: "Mermaid is not defined"**  
**原因：** CDN被阻止或网络问题  
**解决：** 下载Mermaid.js到本地（见下文）

### 步骤2：检查网络请求

1. 在开发者工具中切换到 **Network** 标签
2. 刷新页面 (F5)
3. 查找 `mermaid.min.js` 请求

#### 正常情况
```
mermaid.min.js    200    OK    ~500KB
```

#### 异常情况
```
mermaid.min.js    Failed    (net::ERR_BLOCKED_BY_CLIENT)
```
**解决：** 禁用广告拦截器，或使用本地Mermaid.js

```
mermaid.min.js    Failed    (net::ERR_NAME_NOT_RESOLVED)
```
**解决：** 检查网络连接，或使用本地Mermaid.js

### 步骤3：运行简化测试

运行我们提供的简化测试页面：

```bash
open /tmp/billing-simple-test.html
```

如果简化版能正常显示，说明：
- ✅ 浏览器支持Mermaid
- ✅ 网络连接正常
- ❌ 生成的HTML有问题（请告知我具体错误）

如果简化版也不能显示，说明：
- ❌ 网络问题或CDN被阻止
- ❌ 浏览器JavaScript被禁用
- ❌ 浏览器不兼容

### 步骤4：检查具体哪些图表失败

在控制台中运行：

```javascript
document.querySelectorAll('.mermaid').forEach((el, i) => {
    const svg = el.querySelector('svg');
    console.log(`Graph ${i+1}:`, svg ? '✅ OK' : '❌ FAIL');
});
```

这会告诉你哪些图表渲染成功，哪些失败。

## 解决方案

### 方案1：使用本地Mermaid.js（推荐）

如果CDN被阻止或网络不稳定：

1. 下载Mermaid.js：
```bash
mkdir -p examples/viz-demo/libs
curl -L https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js \
  -o examples/viz-demo/libs/mermaid.min.js
```

2. 修改HTML中的script标签：
```html
<!-- 将这行 -->
<script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>

<!-- 改为 -->
<script src="libs/mermaid.min.js"></script>
```

### 方案2：增加渲染超时

如果图表太复杂导致超时，修改初始化配置：

```javascript
mermaid.initialize({
    startOnLoad: true,
    theme: 'default',
    maxTextSize: 100000,  // 增加文本大小限制
    timeout: 60000        // 增加超时到60秒
});
```

### 方案3：使用静态图片

如果Mermaid始终无法渲染，生成静态SVG：

```bash
# 安装mermaid-cli
npm install -g @mermaid-js/mermaid-cli

# 转换为SVG
mmdc -i goalgraph.mmd -o goalgraph.svg

# 在HTML中使用
<img src="goalgraph.svg" alt="Goal Graph">
```

### 方案4：分别查看每个图表

如果某个特定图表有问题，单独调试：

```bash
# 只生成有问题的类型
intent-lang-visualizer billing.intent --type intent-graph -o test.mmd

# 在Mermaid Live Editor中测试
open https://mermaid.live/
# 复制test.mmd内容到编辑器
```

## 浏览器兼容性

### 推荐浏览器
- ✅ Chrome/Edge 90+ (最佳)
- ✅ Firefox 88+
- ✅ Safari 14+

### 不支持的环境
- ❌ Internet Explorer (任何版本)
- ❌ 禁用JavaScript的浏览器
- ❌ 极旧的浏览器版本

## 特殊字符问题

某些特殊字符可能导致解析错误：

### 需要转义的字符
- `"` → `&quot;`
- `<` → `&lt;`
- `>` → `&gt;`
- `&` → `&amp;`

我们的生成器已经自动处理这些转义，但如果手动修改HTML，请注意。

## 性能问题

如果页面加载很慢：

1. **图表太大**
   - 简化图表（减少节点数量）
   - 分成多个小图表

2. **多个图表同时渲染**
   - 使用标签页（我们已实现）
   - 延迟加载非活动标签的图表

3. **设备性能不足**
   - 使用静态SVG而不是实时渲染
   - 在服务器端预渲染

## 获取帮助

如果以上方法都无法解决问题：

1. 收集以下信息：
   - 浏览器版本和操作系统
   - 控制台完整错误信息
   - Network标签的截图
   - 哪些图表能显示，哪些不能

2. 运行诊断工具：
```bash
/tmp/diagnose-viz.sh > debug-output.txt
```

3. 提供这些信息，我会帮你定位问题

## 快速检查清单

- [ ] 浏览器是否支持JavaScript？
- [ ] 网络连接是否正常？
- [ ] CDN是否被广告拦截器阻止？
- [ ] 控制台是否有错误信息？
- [ ] 简化测试页面是否能正常显示？
- [ ] mermaid.min.js是否成功加载（200状态）？

全部勾选后仍有问题，请提供详细错误信息。
