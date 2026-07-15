# 验收管线示例（M-A1）

演示从 `.intent` 需求到"实现是否满足需求"判定的完整链路
（`docs/rfc-executable-acceptance.md`）。

## 文件

| 文件 | 角色 |
|------|------|
| `transfer.intent` | 需求：子句标签、`modifies` frame、`else reject`、`example` 块、goal |
| `transfer.intent.bind.toml` | binding：抽象需求 ↔ `bank_demo.py` 的语义映射（手写，M-A2 起可由 LLM 起草） |
| `bank_demo.py` | 被测系统：约 50 行内存银行 demo |

## 运行

```bash
# 需求自洽性（目标 A）：Z3 一致性 + reject 分支 + example 代入检查
intent check examples/acceptance/transfer.intent

# 验收（目标 B）：生成 pytest → 执行 → 按子句 ID 归并 → 门禁
intent accept run examples/acceptance/transfer.intent
```

预期：9 个子句级验收项全部 passed，gate[strict] = pass，退出码 0。

## 自举验收标准（埋 bug 演示）

```bash
BANK_DEMO_BUGGY=1 intent accept run examples/acceptance/transfer.intent
```

demo 会多扣 1 元。报告必须把失败**归属到 `TransferSafe/debit` 这条
具体的 ensure 子句**（断言消息内嵌子句 ID），且退出码非零；
未被求值的下游子句如实报 `blocked` 而不是伪装成绿色。

## 产物

默认写到 `intent-accept/`（`--out` 可改）：

- `test_acceptance.py` — 确定性生成的 pytest（勿手改）；
- `acceptance_manifest.json` — 测试 ↔ 子句稳定 ID 映射；
- `acceptance_report.json` — `intent.acceptance_report` 工件
  （schema 见 `docs/protocol/artifacts.md`）。

## 依赖

- `python3` + `pytest`（`pip install pytest`）。
