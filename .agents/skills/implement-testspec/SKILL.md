---
name: implement-testspec
description: 从 .intent 的 testspec 出发落地可执行验收：起草 .intent.bind.toml binding（M-A2 LLM 起草、人工确认），跑 intent accept gen/run，处理 acceptance_report 中的 failed/blocked/manual-pending 项。当用户要"实现测试"、"落地 testspec"、"写 binding"、"跑验收"或验证实现是否满足 .intent 需求时使用。本技能只做验收侧，不修改 .intent 需求文件——需求由 write-intent 技能在独立会话中产出（反勾结原则，见 docs/lang/LLM.md）。
---

# testspec → 可执行验收（binding 起草 + accept 管线）

读 `intent testspec` 的场景草稿，起草 binding，把验收跑起来，按子句 ID
归因结果。执行链路（gen/run）是确定性的；本技能的智能只用在**起草 binding**
和**解读报告**上。

## 边界（反勾结原则）

本技能**只做验收，不改需求**：

- 禁止修改 `.intent` 文件。若发现需求本身可疑（例如公式与业务常识相悖），
  停下来向用户报告，由需求侧（write-intent 技能、独立会话）修正；
- 禁止手改生成的 `test_acceptance.py`（确定性产物，重新 gen 会覆盖）；
- 允许创建/修改：`.intent.bind.toml`、被测代码的测试辅助（fixture）、
  manual 项的手写测试。

## 工作流

```
Task Progress:
- [ ] 1. 读 testspec，弄清要验收哪些场景
- [ ] 2. 读被测代码，起草 binding（人工确认后固化）
- [ ] 3. intent accept gen —— 检查生成的 pytest 合理
- [ ] 4. intent accept run —— 解读 acceptance_report
- [ ] 5. 处理 manual-pending 项（手写测试或向用户要签署）
```

**1. 读 testspec**

```bash
intent --format json testspec <file>.intent
```

每行场景带 `clause_ids`（如 `TransferSafe/debit`）和 `executability`
（`machine` | `manual`）。manual 行（量词子句、不可观测状态）机器管线
覆盖不了，记下来留给第 5 步。

**2. 起草 binding（M-A2 的核心：LLM 起草、人工确认）**

先读被测代码，找到 intent 中每个 type / 状态 / 操作对应的构造器、属性、
函数与异常类型，然后写 `<file>.intent.bind.toml`：

```toml
[meta]
intent_file = "transfer.intent"
adapter = "python-pytest"        # 当前唯一适配器
target = "bank_demo"             # 被测 Python 模块

[types.Account]                  # intent 的 type 怎么构造
construct = "bank_demo.Account(owner={owner}, balance={balance}, active={active})"

[state."Account.balance"]        # 抽象状态怎么读；未声明 = 不可观测 → 人工项
read = "{self}.balance"

[ops.TransferSafe]               # intent 怎么调用
call = "bank_demo.transfer({sender}, {receiver}, {amount})"
reject_signal = "raises"         # else reject 在实现里长什么样：raises | returns_error:<pattern>
error_type = "bank_demo.TransferError"
```

约束：声明式、无控制流——binding 是数据不是程序。ensure 引用的每个状态路径
都要有 `state.*` 条目，否则该子句降级为人工项。
**草稿必须给用户过目确认后再进入下一步**（binding 是进 git 的受审查工件）。

**3. 生成并抽查**

```bash
intent accept gen <file>.intent          # 默认输出到 intent-accept/
```

抽查 `intent-accept/test_acceptance.py`：断言消息应内嵌 `clause <ID>`；
example 块应成为 happy-path 用例。生成失败（退出码 2）通常是 binding 引用
了不存在的 type/op，按报错修 binding。

**4. 执行与解读**

```bash
intent accept run <file>.intent --gate strict
```

读 `intent-accept/acceptance_report.json`（schema 见
`docs/protocol/artifacts.md` 的 `intent.acceptance_report`），按子句状态归因：

| status | 含义 | 动作 |
|--------|------|------|
| `failed` | 实现违反该子句（`detail` 有断言消息与场景） | 是**实现的 bug**，修实现，不许改需求或 binding 来"消音" |
| `blocked` | 同测试内更早断言失败，从未被求值 | 先修上游 failed，重跑 |
| `manual-pending` | 不可机检 | 进第 5 步 |

binding 写错（如 `read` 表达式拼错属性名）也会表现为 failed，先核对
`detail` 里的实际值再下结论。

**5. 人工项**

对每个 manual-pending 子句：能补 `state.*` 观测的补 binding 后重跑；
真不可机检的（量词子句）写独立的手写测试并向用户说明，由用户签署确认。

## 验收标准

`intent accept run --gate strict` 退出码 0（全部子句 passed、无 pending），
或所有剩余 pending 项已向用户交代清楚并获得确认。
