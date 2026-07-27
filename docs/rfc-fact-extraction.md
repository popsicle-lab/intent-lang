# RFC: 从存量代码提取功能点事实（extract-facts 技能与 facts 协议）

Status: Draft
Created: 2026-07-23
Authors: intent-lang contributors
Related:
- `docs/lang/LLM.md`
- `docs/lang/SPEC.md`（`@asis` / `@tobe` 生命周期）
- `docs/rfc-intent-translation-rubric.md`（下称 Rubric RFC）
- `docs/rfc-modeling-integrity.md`
- `.agents/skills/write-intent/SKILL.md`
- `.agents/skills/implement-testspec/SKILL.md`

---

## 1. 摘要

本 RFC 定义链条最上游的第三个 Agent 技能 **`extract-facts`**：
从一个已有项目中提取功能点事实，产出结构化自然语言文档
`<业务域>.facts.md`，作为 `write-intent` 技能的输入。

完整管线：

```text
已有项目（代码 / 测试 / 文档）
  │
  ├─ extract-facts（独立会话，纯实然记录员）
  │      └─ <业务域>.facts.md（原子事实，status: draft）
  │
  ├─ 人工确认关口（逐条 draft → confirmed / rejected）
  │
  ├─ write-intent（独立会话，只翻译 confirmed 条目）
  │      └─ <业务域>.intent（@asis，fact_id 入子句注释）
  │
  └─ implement-testspec（独立会话）
         └─ <业务域>.intent.bind.toml + 可执行验收
```

核心原则：

> **extract-facts 是带笔记本和卷尺的考古学家，不是设计师，也不是评论家。
> 它只记录代码实际做了什么，不推断代码应该做什么。**

facts 条目协议向 Rubric RFC 的 requirement atoms schema（§6.3）对齐，
使"代码 → facts → `@asis` intent"这条链未来可被 `intent-eval`
零语义转换地做忠实度评测。

---

## 2. 动机

`write-intent` 假设输入是一段自然语言需求。但大量真实场景是**存量系统**：
需求文档缺失、过时或与代码不符，唯一可信的"需求"是代码当前的实际行为。
要给这类系统补形式化需求基线（`@asis` 入库，见 SPEC 迁移场景），
必须先回答：

> 这个系统的每个功能点，**现在实际上**是怎么工作的？

直接让 write-intent 会话边读代码边形式化有三个问题：

1. **勾结**：同一上下文里"提取"会不自觉为"好形式化"服务——难以形式化的
   模糊行为被削平，产物退化为形式化草稿而非代码的忠实证词；
2. **不可审**：跳过中间产物，人类失去唯一能低成本全文审核的关口
   （`.intent` 公式的审读成本远高于结构化自然语言）；
3. **不可测**：没有独立的事实真值，Rubric RFC 的忠实度评测
   （atoms ↔ clauses 双向映射）对这条链无从下手。

因此需要一个独立技能，产出独立的、人工可确认的事实工件。

---

## 3. 目标

1. 从指定业务域的存量代码中提取**原子化、带溯源、可人工确认**的功能点事实。
2. 事实文档按 intent-lang 概念骨架组织，使 write-intent 可逐条映射。
3. 每条事实可复验（锚点钉死 commit），可 diff（稳定 fact_id → 漂移报告）。
4. 条目字段与 Rubric RFC 的 atoms schema 对齐，未来评测与遥测零转换消费。
5. 与 write-intent、implement-testspec 保持对称的反勾结会话边界。

## 4. 非目标

1. 不产出 `.intent` 文件（write-intent 的职责）。
2. 不评价代码质量、不推断原始设计意图、不修复疑似 bug。
3. 不做全项目一次性提取（按业务域切片）。
4. 不产出依赖图、技术债台账等架构/管理工件（详见 §12 与旧设计的取舍）。
5. 不定义评测器本身（Rubric RFC 的职责）；本 RFC 只保证产物可被其消费。

---

## 5. 术语

- **功能点事实（fact）**：一条最小、可独立核对的、关于代码当前实际行为的
  自然语言陈述，带代码锚点。对应 Rubric RFC 的 requirement atom 在
  存量代码场景下的前身。
- **业务域（domain）**：一次提取的范围单位，如"订单退款流程"。
- **锚点（anchor）**：`@<pinned-sha>:<相对路径>#L<n>[-L<m>]` 格式的
  可复验代码定位。
- **三区**：facts.md 中互斥的三个条目区——行为事实 / 疑似问题 / 存疑，
  收纳判据见 §7.3。
- **确认关口**：人工逐条把 `status: draft` 翻成 `confirmed` 或 `rejected`
  的审核动作；只有 confirmed 条目允许进入 write-intent。

---

## 6. 立场与来源纪律

### 6.1 纯实然

只记录代码实际行为。遇到疑似 bug、未做的边界检查、与注释矛盾的实现：
**标记，不改写，不裁决**。裁决权属于 stakeholder（在确认关口行使，
或经 write-intent 的 V0020 矛盾暴露机制行使）。

这是 write-intent 质量规则"只翻译、不自修复"在提取侧的镜像，也与
Rubric RFC §8.6 的歧义保持维度同构：没有人工依据时擅自选择解释是失真。

### 6.2 来源分级

| 来源 | 用途 | 权威级 |
|------|------|--------|
| 源代码 | 行为事实的**唯一**依据 | 权威 |
| 测试代码 | 行为佐证（真的执行过）；真实业务值 → example 候选 | 佐证 |
| 文档 / 注释 / README | 术语表、业务语境、命名 | 参考，**不作为行为事实** |
| git 历史 | TODO/FIXME 年龄（blame）、变更热点 | 参考 |

来源之间矛盾（文档说"余额不能为负"但代码没检查）→ 如实记录为
疑似问题区的**来源冲突**条目，用 `relations.conflicts_with` 互指，不调和。

### 6.3 运行边界

静态阅读为主。允许：跑项目已有测试套件、用 REPL/脚本调用纯函数做
只读行为探针——运行观察到的行为在条目 evidence 中标 `[运行验证过]`。
禁止：修改被提取项目的任何代码、任何写入性/破坏性操作（改数据库、发请求、
插桩加日志）。

分析工具不可用时允许降级到低精度方法，但必须在 meta 中记录，
且受影响章节标 `[reduced fidelity]`——永远不悄悄替换。

---

## 7. 产物协议：`<业务域>.facts.md`

单一 markdown 文件，与未来的 `<业务域>.intent`、`<业务域>.intent.bind.toml`
同目录并排（三件套）。散文区（术语表、状态流转综述）用自然 markdown；
事实条目用**刚性字段格式**（一行一字段、字段名固定），保证确定性解析器
可做 md → atoms.yaml 的纯机械转换（换衣服，不含语义解释）。

### 7.1 meta 头（版本包）

对齐 Rubric RFC §17.1 的不可变版本包思想：

```markdown
## Meta

- domain: 订单退款流程
- domain_abbrev: RF            <!-- fact_id 用，一经选定不改 -->
- pinned: myproject@a1b2c3d    <!-- 锚点回退阶梯见 §7.4 -->
- extracted_at: 2026-07-23
- skill_version: extract-facts/0.1.0
- tools: rg ✓, tokei ✗ [reduced fidelity: LoC 靠 wc 估算]
```

### 7.2 文档骨架

按 intent-lang 概念组织，write-intent 可逐节映射：

| 章节 | 对应 intent-lang 概念 | 形态 |
|------|----------------------|------|
| Meta | — | 字段表 |
| 术语表 | 命名参考 | 散文表格 |
| 实体与状态 | `type` / `enum` | 结构描述 + 锚点 |
| 状态流转 | require 源态 → ensure 次态 | 流转表 + 综述 |
| 操作（每操作一节） | `intent` | 分组容器 + 原子事实（§7.3） |
| 全局不变量 | `safety` | 原子事实（scope: global） |
| 疑似问题区 | —（交 stakeholder） | 原子事实 |
| 存疑区 | —（人工待办） | 原子事实（无锚点） |
| Extraction Checklist | — | 自检清单 |

### 7.3 双层条目结构

**操作条目**是分组容器：头部放签名、主锚点、一句话职责描述、
example 候选（来自测试的真实业务值）。容器内分三栏——
**前置检查 / 状态效果 / 错误路径**——每栏内的每条承诺是一条
**原子事实**（对齐 atoms 质量要求 §6.4：一条只表达一个可独立核对的承诺）。

原子事实的固定字段：

| 字段 | 取值 | 说明 |
|------|------|------|
| `fact_id` | `F-<域缩写>-<类别>-NNN` | 稳定 ID；类别 = BEH（行为）/ SUS（疑似问题）/ UNK（存疑） |
| `statement` | 一句原子化自然语言 | 保留条件、结果、否定、边界方向 |
| `modality` | `must / must_not / may / should / (unknown)` | 按**代码强制执行的方向**标注（有 reject 分支 → must；无检查 → 不发明条目） |
| `status` | `draft / confirmed / rejected` | 提取时一律 draft；确认关口翻转 |
| `source` | 锚点（§7.4） | 行为事实必填；写不出 → 条目降入存疑区 |
| `evidence` | 签名 / 代码片段 / 测试名 / 注释原文 | 中性证据，不加评判；运行验证过则标注 |
| `relations` | `conflicts_with: [fact_id...]` | 仅在真有冲突时写 |

三区收纳判据（互斥）：

- **行为事实**：有锚点，中性记录代码行为；
- **疑似问题**：有锚点，但行为可疑——panic/unwrap 热点、TODO/FIXME
  承认的缺口（附 blame 年龄）、来源冲突、与注释矛盾的实现；
- **存疑**：写不出锚点，或某字段只能逐字写 `(unknown — needs human input)`。
  该哨兵串是下游 flag，禁止用猜测填充。

### 7.4 锚点与回退阶梯

标准格式 `@<pinned-sha>:<相对路径>#L<n>[-L<m>]`。回退阶梯：

1. 干净 git 工作区 → HEAD sha；
2. 脏工作区 → `<sha>-dirty`，meta 记录；
3. 非 git 项目 → 用 `extracted_at` 日期替代 sha，全文标 `[reduced fidelity]`。

**漂移检测**：换 pinned commit 重跑提取 → diff 新旧 facts 的
fact_id/source/statement，即得过期条目清单 → 反查引用这些 fact_id 的
`.intent` 子句（fact_id 在子句注释里，见 §9），即得过期需求清单。
事实基由此从一次性快照变成可重跑对账的活契约。

### 7.5 完成判据（Definition of Done）

- 每个操作条目的三栏，每栏**至少一条原子事实**，或逐字 unknown 哨兵——
  不许留空、不许删栏；
- 粗扫清单（§8）中列出的每个操作：要么有操作条目，要么在文档尾部
  显式记录"未覆盖及原因"；
- 每条行为事实都有锚点；每个 `(unknown)` 都在存疑区有对应条目；
- Extraction Checklist 全勾。

### 7.6 条目示例

```markdown
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
```

---

## 8. 工作流

```text
Task Progress:
- [ ] 1. 定范围：用户给定业务域；未给则粗扫列业务域清单让用户挑
- [ ] 2. 深挖读码：入口 → 调用链 → 状态字段 → 检查/异常分支
- [ ] 3. 佐证：跑已有测试，收割真实业务值为 example 候选
- [ ] 4. 写 facts.md：meta → 骨架各节 → 三区条目（全部 status: draft）
- [ ] 5. 自检 checklist → 停，提示用户进入确认关口
```

粗扫（step 1）产出的业务域清单本身是与用户对齐范围理解的检查点。
一次只做一个业务域，一域一份文档。

### 8.1 记录员三反模式

（吸收自早期 fact-extractor 设计，实践验证过的失败模式。）

**发表观点**
❌ "auth 模块设计糟糕，关注点混杂。"
✅ "模块 auth（src/auth/，1,243 LoC）导出 14 个公开函数；其中 7 个通过
lazy_static 修改全局状态（@sha:src/auth/state.rs#L22）。"

**推断意图**
❌ "process_payment 应该在扣款前校验金额。"
✅ "process_payment 对 amount 无前置检查（@sha:src/payment.rs#L108）；
其上方 TODO 注释写：`// TODO: validate amount > 0`。"（记录缺口 + TODO
原文，不声称 TODO 是对的——这条进疑似问题区。）

**数字近似**
❌ "大约 30% 的代码在 core 里。"
✅ "tokei 报告 core 4,127 LoC / 总 13,508 LoC（30.5%）。"

---

## 9. 反勾结边界与生命周期

### 9.1 会话边界

与既有两技能对称，三段独立会话：

| 技能 | 允许产出 | 禁止 |
|------|---------|------|
| extract-facts | `<域>.facts.md` | 创建/修改 `.intent`、binding、测试 |
| write-intent | `.intent` | 修改 facts.md 的 confirmed 条目、binding、测试 |
| implement-testspec | binding、验收 | 修改 `.intent`、facts.md |

角色同构于 Rubric RFC §12.1：extract-facts ≈ "atoms 提取模型 B"
（只提取、不翻译）；write-intent ≈ 翻译模型 A。同一会话既提取又形式化，
提取会不自觉为"好形式化"服务——这是必须隔离的失真源。

### 9.2 确认关口

人工逐条审核 facts.md：

- `confirmed`：认可为需求真值，允许进入 write-intent；
- `rejected`：stakeholder 判定该行为是 bug，**不**升格为需求
  （修 bug 属于 `@tobe` 新承诺，另走 write-intent 正向流程）；
- 保持 `draft`：暂不处理。

对齐 Rubric RFC §6.3："只有 confirmed atoms 参与正式评测"。

### 9.3 write-intent 侧衔接（对其 SKILL.md 的修改）

给 `.agents/skills/write-intent/SKILL.md` 增加一小节（≤15 行），要点：

1. 输入是 facts.md 时**只翻译 `status: confirmed` 条目**；
2. 按骨架逐节映射（实体→type、操作→intent、前置检查→require、
   状态效果→ensure、全局不变量→safety）；
3. `fact_id` 写进对应子句的注释，建立 clause ↔ fact 可审计映射；
4. `conflicts_with` 互指的条目**如实翻译成并列子句**，让
   `intent check` 用 V0020 暴露（对齐 Rubric RFC §11.1 预期诊断匹配：
   `expected_consistency: contradictory`）；
5. 生命周期：默认全部标 `@asis`，验证闭环用
   `intent check --include-asis` 跑；stakeholder 审定 `.intent` 后，
   认可的条目升级为无标注（当前规则），要改造的另写 `@tobe`。

衔接规则必须写在 write-intent 自己的文档里——反勾结要求它在独立会话
加载，上游文档它看不见。

---

## 10. 与评测 / 遥测的对齐

本 RFC 的产物协议使 Rubric RFC 的机制可直接复用于存量代码链路：

| Rubric RFC 概念 | 本 RFC 对应物 |
|-----------------|--------------|
| requirement atom（§6.3） | facts.md 原子事实（字段对齐：id/statement/modality/scope/source_evidence/status/relations） |
| 人工确认 atoms | 确认关口（draft → confirmed/rejected） |
| atoms 提取模型 B（§12.1） | extract-facts 独立会话 |
| atom ↔ clause 双向映射（§7） | fact_id ↔ 子句注释 |
| 冲突保持（§8.5）+ 预期诊断匹配（§11.1） | `conflicts_with` → 并列子句 → V0020 |
| `intent-eval --atoms *.atoms.yaml`（§14.2） | 确定性 md→yaml 解析器（未来，纯机械转换） |

可算的遥测指标（对齐 Rubric RFC §19）：fact 覆盖率（confirmed fact 有无
对应子句）、clause groundedness（子句有无 fact 依据 → 检测臆造）、
冲突保持率、`(unknown)` 比例及原因分布、确认关口的逐条耗时、
rejected 比例（≈ 存量代码的"bug 密度"信号）、漂移 diff 规模。

刻意不做的：不在提取阶段给事实"打分"（没有独立真值可对照，打分即观点）；
不让 extract-facts 会话自评忠实度（同 Rubric RFC 禁止翻译会话自评）。

---

## 11. 技能文件设计

- 位置：`.agents/skills/extract-facts/SKILL.md`，单文件自足（与兄弟技能
  一致，不外置 reference），目标 ≤240 行；
- description：WHAT + WHEN 触发词（"提取功能点""逆向需求""补需求基线"
  "从代码提取事实"）+ 反勾结边界句（"只产 facts.md，不写 .intent"）；
  不设 `disable-model-invocation`，靠 description 自动触发；
- 结构：边界（反勾结）→ facts.md 模板（内联，含 §7.6 式填好范例）→
  工作流（Task Progress checklist）→ 质量规则（完成判据、三反模式、
  unknown 哨兵、降级声明）→ 收尾提示（引导用户进确认关口 + 新会话
  write-intent）；
- 分语言扫描命令压缩到数行提示（agent 本来会用 rg/tokei），
  不复制早期设计的完整命令清单。

## 12. 与早期设计（popsicle fact-extractor）的取舍

| 早期机制 | 取舍 | 理由 |
|----------|------|------|
| fact_id + pinned-sha 溯源 + 漂移 diff | **采纳** | 最值钱的两个机制，直接补强溯源与活契约 |
| 三反模式 ❌/✅ 对照、unknown 哨兵、降级声明、checklist | **采纳** | 实践验证过的失真防线 |
| 从测试收割行为事实 / golden 候选 | **采纳**（变体） | → example 候选，喂 write-intent 质量规则 4 |
| facts.yaml + 5 份 markdown 渲染双产物 | 不采纳 | 双产物同步腐烂；我们的下游是人 + LLM 会话，单一 markdown 够 |
| 依赖图 / API 契约 / 技术债 / 汇总报告四件套 | 不采纳 | 服务架构设计与管理汇报，write-intent 用不上；unsafe/panic/TODO 并入疑似问题区 |
| workflow 状态机 + guard | 不采纳 | Cursor 技能无 workflow 引擎；人工关口由 status 字段承担 |

---

## 13. 待决问题

1. md → atoms.yaml 确定性解析器何时落地（`intent-eval` 原型需要时）？
2. `domain_abbrev` 冲突（多业务域缩写撞车）是否需要仓库级注册表？
3. 跨业务域共享的实体（如 Account 被转账和退款共用）事实归属哪份文档？
4. 漂移 diff 是否值得做成 CLI 子命令（`intent facts diff`），
   还是保持技能内的人工流程？
5. rejected 条目的后续追踪（bug 台账）放在哪——facts.md 内、
   issue tracker、还是不管？

## 14. 决策摘要

- 纯实然立场：只记录代码实际行为，疑似 bug 标记不改写；
- 产物为单一结构化自然语言 markdown，零 intent 语法，刚性条目字段格式；
- 代码为行为事实唯一权威；测试供佐证与 example 候选；文档供术语；
- 双层条目：操作容器 + 原子事实；字段对齐 Rubric RFC atoms schema
  （fact_id / statement / modality / status / relations / source / evidence）；
- 三区互斥：行为事实 / 疑似问题 / 存疑（unknown 哨兵）；
- 强制溯源：`@<pinned-sha>:路径#L` + 回退阶梯 + 漂移 diff；
- 完成判据：每操作每栏至少一条原子事实或 unknown 哨兵；
- 一次一业务域；粗扫 → 用户挑 → 深挖；
- 三技能独立会话反勾结；人工确认关口 = status 翻转，机器可读；
- write-intent 只翻译 confirmed 条目，fact_id 入子句注释，
  冲突如实并列交 V0020，默认 `@asis` + `--include-asis`；
- 静态为主，可跑已有测试与只读探针，禁改代码；
- 工具降级必须声明 `[reduced fidelity]`。

评测链路（对齐 Rubric RFC 的证据链）：

```text
代码证据（锚点）
  → 人工确认 fact（confirmed atom）
  → @asis intent clause（fact_id 注释）
  → 忠实度 finding / 遥测指标
  → 独立形式质量与 Z3 诊断
```
