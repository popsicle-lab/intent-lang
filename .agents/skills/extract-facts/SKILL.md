---
name: extract-facts
description: 从已有项目提取功能点事实，产出 <业务域>.facts.md——原子化、带代码锚点、可人工逐条确认的实然事实清单，作为 write-intent 的输入。当用户要"提取功能点"、"逆向需求"、"从代码提取事实"、"给存量项目补需求基线"时使用。本技能是纯实然记录员：只记录代码实际行为，不写 .intent、不评价代码、不推断意图——形式化由 write-intent 技能在独立会话完成（反勾结原则）。
---

# 存量项目 → 功能点事实（facts.md）

> **自包含**：本技能装到任意外部项目即可用。正文即全部规范，不要去读、引用或依赖
> intent-lang 源码仓库里的 RFC、SPEC、示例或其它文件。

把一个已有项目的指定业务域变成**事实基**。你是带笔记本和卷尺的考古学家，
**不是**设计师，**也不是**评论家。

```text
已有项目 → extract-facts（本技能）→ <业务域>.facts.md（全部 status: draft）
        → 人工确认（draft → confirmed / rejected / deferred）
        → write-intent（新会话，只翻译 confirmed，产 @asis intent）
        → intent trace（机器核对：漏译 / 悬空引用 / 未裁决的 SUS·UNK）
        → implement-testspec（新会话）
```

## 边界（反勾结原则）

本技能**只提取事实，不形式化、不修代码**：

- 只允许产出 `<业务域>.facts.md`（与未来的 `.intent`、`.intent.bind.toml`
  同目录并排）；禁止创建/修改 `.intent`、binding、测试代码；
- 禁止修改被提取项目的任何代码，禁止写入性操作（改数据库、发请求、插桩）；
  允许跑项目**已有**测试套件和只读探针（REPL 调纯函数），观察到的行为在
  evidence 里标 `[运行验证过]`；
- 产完即停：提示用户逐条审核 status，然后在**新会话**用 write-intent。
  同一会话既提取又形式化，提取会不自觉为"好形式化"服务——削平模糊行为，
  产物退化成形式化草稿而不是代码的忠实证词。

## 记录员纪律（三反模式）

**1. 发表观点**
❌ "auth 模块设计糟糕，关注点混杂。"
✅ "模块 auth（src/auth/，1,243 LoC）导出 14 个公开函数；其中 7 个通过
lazy_static 修改全局状态（@sha:src/auth/state.rs#L22）。"

**2. 推断意图**
❌ "process_payment 应该在扣款前校验金额。"
✅ "process_payment 对 amount 无前置检查（@sha:src/payment.rs#L108）；
其上方 TODO 注释写：`// TODO: validate amount > 0`。"
（记录缺口 + TODO 原文，不声称 TODO 是对的——这条进疑似问题区。）

**3. 数字近似**
❌ "大约 30% 的代码在 core 里。"
✅ "tokei 报告 core 4,127 LoC / 总 13,508 LoC（30.5%）。"

另外两条铁律：

- 不知道就**逐字**写 `(unknown — needs human input)`，禁止猜——下游把这串
  字符当 flag 去问人；
- 分析工具不可用时可降级到低精度方法，但必须在 Meta 记录，受影响章节标
  `[reduced fidelity]`——永远不悄悄替换。

## 来源分级

| 来源 | 用途 | 权威级 |
|------|------|--------|
| 源代码 | 行为事实的**唯一**依据 | 权威 |
| 测试代码 | 行为佐证（真的执行过）；真实业务值 → example 候选 | 佐证 |
| 文档 / 注释 / README | 术语表、业务语境、命名 | 参考，不作为行为事实 |
| git 历史 | TODO/FIXME 年龄（blame）、变更热点 | 参考 |

来源矛盾（文档说"余额不能为负"但代码没检查）→ 如实记录为疑似问题区的
来源冲突条目，`conflicts_with` 互指，**不调和、不裁决**。

## 工作流

```text
Task Progress:
- [ ] 1. 定范围：用户给定业务域；未给则粗扫列业务域清单让用户挑
- [ ] 2. 深挖读码：入口 → 调用链 → 状态字段 → 检查/异常分支
- [ ] 3. 佐证：跑已有测试，收割真实业务值为 example 候选
- [ ] 4. 写 facts.md：Meta → 骨架各节 → 三区条目（全部 status: draft）
- [ ] 5. 自检 Extraction Checklist → 停，引导用户进确认关口
```

**1. 定范围**：一次只做一个业务域，一域一份文档。粗扫用 `rg`/`tokei`/目录
结构识别业务域候选（顶级模块、路由分组、服务边界），列清单让用户挑——
这份清单同时是和用户对齐范围理解的检查点。

**2. 深挖**：从操作入口（API handler、公开函数、命令）出发追调用链，
重点找：状态字段及其取值、每个检查/拒绝分支、每处状态写入、抛出的异常
类型、panic/unwrap 热点、TODO/FIXME（用 `git blame` 记年龄）。

**3. 佐证**：跑已有测试套件；测试里的输入/期望值是现成的真实业务值，
记为操作条目的 example 候选（下游 write-intent 质量规则 4 需要）。

**4-5.** 按下面的模板写文档，勾完 checklist 即停。

## facts.md 模板

### 锚点格式与回退阶梯

标准锚点 `@<pinned-sha>:<相对路径>#L<n>[-L<m>]`。回退：
干净 git 工作区 → HEAD sha；脏工作区 → `<sha>-dirty` 并在 Meta 记录；
非 git 项目 → 用提取日期替代 sha，全文标 `[reduced fidelity]`。

### 三区判据（互斥）

- **行为事实**（`F-<域>-BEH-NNN`）：有锚点，中性记录代码行为；
- **疑似问题**（`F-<域>-SUS-NNN`）：有锚点但行为可疑——panic/unwrap 热点、
  TODO/FIXME 承认的缺口（附 blame 年龄）、来源冲突、与注释矛盾的实现；
- **存疑**（`F-<域>-UNK-NNN`）：写不出锚点，或字段只能写 unknown 哨兵。

### 原子事实字段（刚性格式：一行一字段，字段名固定）

| 字段 | 取值 |
|------|------|
| `fact_id` | `F-<域缩写>-<BEH\|SUS\|UNK>-NNN`，稳定不复用 |
| `statement` | 一句原子化陈述：只表达一个可独立核对的承诺，保留条件、否定、边界方向 |
| `modality` | `must / must_not / may / should / (unknown)`——按**代码强制执行的方向**标（有 reject 分支 → must；无检查 → 不发明条目） |
| `status` | 提取时一律 `draft`；确认关口由人翻成 `confirmed`（认可为需求真值）/ `rejected`（判定为 bug，不进需求）/ `deferred`（已看过，本轮不裁决）。这四个取值是刚性的，`intent trace` 只认它们，写别的会被报为解析告警 |
| `source` | 锚点；行为事实必填，写不出 → 条目降入存疑区 |
| `evidence` | 签名 / 代码片段 / 测试名 / 注释原文，中性不评判；运行验证过则标注 |
| `relations` | `conflicts_with: [fact_id...]`，仅真有冲突时写 |

### 文档骨架与条目范例

````markdown
# <业务域> 功能点事实

## Meta
- domain: 订单退款流程
- domain_abbrev: RF            <!-- fact_id 用，一经选定不改 -->
- pinned: myproject@a1b2c3d
- extracted_at: 2026-07-23
- skill_version: extract-facts/0.1.0
- tools: rg ✓, tokei ✗ [reduced fidelity: LoC 靠 wc 估算]

## 术语表
| 术语 | 含义（来自文档/注释） |

## 实体与状态
- Order：字段 status ∈ {Paid, Refunded, Closed}（@a1b2c3d:src/models.py#L12-L20）

## 状态流转
| 源态 | 操作 | 次态 | 锚点 |
| Paid | refund | Refunded | @a1b2c3d:src/refund/service.py#L83 |

## 操作

### 操作：refund(order_id, amount)
- source: @a1b2c3d:src/refund/service.py#L41-L88
- 职责：对已支付订单发起退款
- example 候选: order.amount=500, refund.amount=500 → status=Refunded
  （来自 @a1b2c3d:tests/test_refund.py#L23 [运行验证过]）

#### 前置检查
- fact_id: F-RF-BEH-001
  statement: 退款金额大于订单实付金额时拒绝退款并抛 RefundExceedsPayment
  modality: must
  status: draft
  source: @a1b2c3d:src/refund/service.py#L47-L49
  evidence: `if amount > order.paid_amount: raise RefundExceedsPayment`

#### 状态效果
- fact_id: F-RF-BEH-002
  statement: 退款成功后订单状态从 Paid 变为 Refunded
  modality: must
  status: draft
  source: @a1b2c3d:src/refund/service.py#L83
  evidence: `order.status = OrderStatus.REFUNDED`

#### 错误路径
- fact_id: F-RF-UNK-001
  statement: (unknown — needs human input) 并发重复退款时的行为未能确定
  modality: (unknown)
  status: draft
  source: —（见存疑区）
  evidence: 代码无锁/幂等键；测试无并发用例

## 全局不变量
（scope 为全域的原子事实，如 DB 约束、全局校验；格式同上）

## 疑似问题区
（SUS 条目：panic/TODO/来源冲突；来源冲突用 conflicts_with 互指）

## 存疑区
（UNK 条目：无锚点或含 unknown 哨兵；每个正文中的 (unknown) 在此有对应项）

## 未覆盖操作
（粗扫清单中有、本次未深挖的操作及原因）

## Extraction Checklist
- [ ] 每个操作条目三栏（前置检查/状态效果/错误路径）每栏至少一条原子事实或 unknown 哨兵
- [ ] 粗扫清单中每个操作：有条目，或记入"未覆盖操作"
- [ ] 每条行为事实都有锚点；每个 (unknown) 在存疑区有对应条目
- [ ] 所有条目 status: draft；无观点、无意图推断、无近似数字
- [ ] 工具降级已在 Meta 记录并标 [reduced fidelity]
- [ ] 会话末尾已单列「待裁决清单」（全部 SUS + UNK）
````

## 完成后

### 必须输出「待裁决清单」

在会话末尾，把**所有 SUS 与 UNK 条目单独列成一张表**给用户，不要让它们埋在
几百行文档里等人自己翻。BEH 条目量大且多数会被整体认可，SUS/UNK 才是真正需要
逐条判断的：

```text
待裁决（4 条 SUS / 2 条 UNK）——逐条给出 confirmed / rejected / deferred
  F-RF-SUS-001  Kafka 消息 country 字段写入的是 siteCode，与文档描述不一致
  F-RF-SUS-002  ...
  F-RF-UNK-001  (unknown) 并发重复退款的行为无法从代码确定
```

`deferred` 是"看过了，本轮不裁决"，与 `draft`（没人看过）不同。下游
`intent trace` 会拦截仍是 `draft` 的 SUS/UNK——确认关口没走完，形式化就不该开始。
给了 `deferred` 就放行，所以这是个关口，不是死锁。

### 告诉用户

1. 逐条审核 facts.md——`confirmed`（认可为需求真值）/ `rejected`
   （判定是 bug，不升格为需求；修复属于 `@tobe` 新承诺另走正向流程）/
   `deferred`（本轮不裁决）；
2. 审完在**新会话**用 write-intent 技能形式化（它只翻译 confirmed 条目）；
3. 将来代码演进后：换 pinned commit 重跑本技能，diff 新旧 facts 的
   fact_id/source/statement 即得过期条目清单，反查 `.intent` 子句注释里的
   fact_id 即得过期需求清单。
