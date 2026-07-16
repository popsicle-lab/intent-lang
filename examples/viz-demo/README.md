# Intent-Lang 可视化示例

由 `intent-lang-visualizer` 从 `.intent` 需求文件自动生成的交互式可视化，每个例子一个自足的 `index.html`：状态机、目标追溯、安全规则清单、覆盖场景备忘、带高亮的源码，点节点/行看完整契约。生成方式见每个例子内的说明。

## 例子

| 例子 | 源模型 | 打开方式 |
| --- | --- | --- |
| [ticket/](./ticket/) | [`examples/requirements/ticket.intent`](../requirements/ticket.intent) | [`ticket/index.html`](./ticket/index.html) · [导读](./ticket/README.md) |
| [billing-all/](./billing-all/) *（旧版模板）* | [`examples/requirements/billing.intent`](../requirements/billing.intent) | [`billing-all/index.html`](./billing-all/index.html) |
| [smarthome-all/](./smarthome-all/) *（旧版模板）* | [`examples/smarthome/smarthome.intent`](../smarthome/smarthome.intent) | [`smarthome-all/index.html`](./smarthome-all/index.html) |

`ticket/` 用的是重组后的新模板（单页、按业务优先排 tab、点击开详情面板、pan/zoom）；`billing-all/` 与 `smarthome-all/` 还是旧模板（多 tab 图表 + 大表格，无点击详情面板），尚未重新生成——不是 bug，只是还没轮到。用下面的命令即可升级到新模板：

```bash
cargo run -p intent-lang-visualizer -- examples/requirements/billing.intent   --all --output-dir examples/viz-demo/billing-all
cargo run -p intent-lang-visualizer -- examples/smarthome/smarthome.intent    --all --output-dir examples/viz-demo/smarthome-all
```

## 用法

```bash
# 完整套件（.mmd 导出 + 交互式 index.html）
cargo run -p intent-lang-visualizer -- <file>.intent --all --output-dir <dir>

# 只要单文件交互页（等价于上面的 index.html，不落 .mmd）
cargo run -p intent-lang-visualizer -- <file>.intent --interactive -o <file>.html
```

其余可视化类型（如已退役出默认输出的安全规则二部图）仍可通过 `--type` 显式生成，见 `cargo run -p intent-lang-visualizer -- --help`。
