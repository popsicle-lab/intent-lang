# RFC: 可执行验收管线（executable acceptance pipeline）

Status: M-A1 Implemented；M-A2 以 Agent Skills 形态落地
（`.agents/skills/write-intent`、`.agents/skills/implement-testspec`，
不再依赖 PLAN M4 的 intent-llm 基础设施）；M-A3 第二适配器待定
Created: 2026-07-15
Implemented: 2026-07-15 —— `intent-lang-accept` crate、`intent accept gen/run`、
`.intent.bind.toml`、Z3 见证求解、`intent.acceptance_report` 工件与严格/宽松
门禁均已落地；自举验收标准（埋 bug 归属到 `TransferSafe/debit`）通过，
见 examples/acceptance/ 与 crates/intent-lang-accept/tests/pipeline.rs；
visualizer 验收报告渲染按计划后置到 M-A1.5
Context: 来自一次针对"实现需求建模语言，并且可验收"目标的系统性拷问（grilling session），
共 11 个决策逐一确认后收敛而成
Related: docs/rfc-idd-positioning-and-improvements.md, docs/rfc-modeling-integrity.md,
docs/protocol/artifacts.md, docs/lang/POSITIONING.md, PLAN.md
Amended: 2026-07-15 —— rfc-modeling-integrity.md 的 8 项决策修订了本文的
D7 执行细节、4.1 的 ID 方案，并关闭了 open question 1 与大部分 3

---

## 0. 一句话主张

> 把不确定性全部挤压到**生成时**，**执行时**必须完全确定 ——
> 因为验收报告要能当合同证据用。

本 RFC 的每一个设计决策都服从这条原则。

---

## 1. Motivation

intent-lang 当前实现的是"需求本身可验收"：Z3 一致性检查、coverage、diff/impact。
这回答了"需求写得对不对"，但没有回答用户真正的问题：

> **实现是否满足需求？**

`intent testspec` 输出的场景行（`testspec.draft` 工件）是这条链路的起点，
但它依赖外部工具消费，仓库内没有任何东西能把它变成"通过 / 不通过"的判定。
只停在这里，intent-lang 就只是一个"需求 linter"，与 Alloy / TLA+ 抢地盘且抢不过。

本 RFC 定义从 `.intent` 到可执行验收判定的完整管线，即**目标 B**：

```
目标 A（已有）：需求自洽 —— parse → typeck → VCGen → Z3 一致性
目标 B（本 RFC）：实现可对需求验收 —— testspec → binding → codegen → 执行 → 归属报告
```

目标 A 是目标 B 的前置门槛，不是终点。

---

## 2. 决策记录（11 项，已确认）

以下每一项都是经过逐一质询后由项目作者确认的决策，含被否决的备选项。

### D1. "可验收"的含义 = 实现可对需求验收

- 否决：仅"需求文档质量可验收"（现状，不够）；仅"交付过程可验收"（diff/impact 是配菜不是主菜）。

### D2. 被测系统不设限 → 语言级适配器架构

被测系统"可能是任何程序"。"任何程序"与"可执行验收"天然互斥，
唯一体面的解法是 **intent-lang 定义协议、执行交给适配器**（类比 LSP：
不实现任何编辑器，但定义编辑器与语言服务器之间的合同）。

铁律：**没有参考实现的协议是废纸**，见 D5。

- 否决：HTTP-only（覆盖不了库 / 算法 / 无服务边界的程序）。

### D3. binding 由 LLM 起草

binding = 抽象需求 ↔ 具体代码的语义映射（`transfer` 操作对应哪个函数、
`balance` 状态怎么读取、账户 fixture 怎么构造、require 违反在该语言里是
返回错误还是抛异常）。这些知识是**每个项目一份**的，不是每个语言一份的 ——
做十个语言适配器也不会让任何一个用户少写一行 binding。

LLM 用于摊薄冷启动成本。

- 否决作为主路线：纯手写（成本高）、注解/属性宏（每语言一套且需求方看不到全貌）、
  命名约定（真实项目命名从不听话）。手写永远是兜底。

### D4. binding 必须固化为确定性工件

LLM 起草 → 写成确定性文件 → 人工审查 → 进 git → **执行时零 LLM 介入**。
LLM 只在两个时机出现：冷启动生成、`intent diff` 检测到需求变更后的增量更新草稿。

- 否决：执行时现场推断（同一份代码 + 同一份需求，今天通过明天失败，
  报告失去合同证据资格）。

### D5. 第一个参考适配器 = Python + pytest

- 理由：LLM 对 Python 的推断质量最高（放大 D3 路线的优势）；动态类型使
  binding 胶水代码最薄；pytest 的失败输出可机读，验收报告可直接建在其上。
- Rust 自举留给第二个适配器，用来证明协议真的语言无关。

### D6. LLM 职责压缩到最小面：只产 binding，codegen 全确定性

固化的工件是 **binding 声明文件**，不是 LLM 直接生成的测试代码。
测试代码由 intent-lang 的 Rust 生成器读 testspec + binding **机械产出**：

- 断言逻辑（`assert post == pre - amount`）永远由确定性代码翻译，忠实性可审计；
- 需求变更时 binding 大概率不变（函数没换，只是条件改了），重新 codegen 即可，零 LLM；
- LLM 的输出面只剩"命名映射 + fixture 构造"——恰好是 LLM 最不容易错、
  错了人也最容易审出来的部分。

- 否决：LLM 直接产测试代码（① 增量重生成会丢用户手工修改；
  ② "测试断言 = 需求子句的忠实翻译"无法保证，不确定性只是换了位置）。

### D7. 子句级可执行性静态分类；不可机检 → 显式人工验收项

Z3 链路是符号推理，验收执行是具体运行，两者能力边界不重合。
每条子句在 testspec 阶段被确定性地静态判定：**可机检 / 不可机检**。

- 不可机检的子句（无界全称量词、存在量词、binding 未声明可观测的状态）
  进入验收报告的**人工检查清单**，与机检项分开计数，**绝不静默跳过**，
  绝不合并出虚假的"100% 通过"；
- typecheck 对不可执行子句发 warning（类似 W0010），作者写的时候就知情；
- 无界 `forall` 在验收里做成"对当前可枚举实例的抽样检查"，
  报告如实标注为 `sampled`——比假装是证明诚实，比拒绝它实用。
  **修订（rfc-modeling-integrity D6/D7）**：语言目前没有"当前实例集合"
  概念，`sampled` 语义推迟到 `state` 块（独立 RFC，语言 0.2）落地后启用；
  在此之前量词子句一律归 `manual-pending`，不建过渡性枚举机制。

- 否决：编译期拒绝不可执行子句（伤表达力，全局不变量天然该用量词写）；
  静默跳过（陷阱，直接摧毁报告可信度）。

### D8. 测试输入由 Z3 求解见证值产生

happy 输入 = 满足 require 的模型；负面输入 = 满足 ¬require 的模型；
边界值 = 对每个原子条件求等号成立点的模型（如 `balance == amount`）。

这是全项目最重要的复用：**同一个 Z3 既做需求一致性检查（目标 A）
又做验收数据生成（目标 B）**，两条产品线共享核心。这是相对
"Gherkin + 手写测试"的真正技术护城河。`smt.rs` 已经在产 counterexample
模型，只是方向反过来用。

Z3 说 `balance = 100`；binding 说怎么造一个余额 100 的账户 —— 一层解一层。

- 否决作为第一版：binding 写死 fixture 数据（边界值靠拍脑袋，require 改后
  旧数据悄悄失效，仅作兜底）；property-based / hypothesis（远期增强，不进第一版）。

### D9. 子句稳定 ID 贯穿全链；自有 JSON 验收工件

报告的主语必须是**需求子句和 goal**，不是测试函数 —— 这是验收与普通测试的本质区别。

- 每条 require / ensure / invariant / safety 子句获得稳定 ID；
- codegen 把 ID 埋进生成的测试名；
- `intent accept run` 跑完 pytest 后读 JUnit XML，按 ID 归并回需求侧，
  产出 JSON 验收工件（新增 `intent.acceptance_report` kind，挂进
  docs/protocol/artifacts.md 协议）；
- 沿 `realized_by` 上卷到 goal 层："目标 G1：机检 8/10 通过，2 项待人工确认"；
- visualizer 负责 HTML 渲染（给非程序员的验收方看）。

隐藏红利：`intent diff` 已做子句级变更检测，ID 打通后
"需求变更 → 哪些验收结果失效 → 只重跑这些"是免费的。

- 否决：直接用 JUnit XML 当报告（主语错位，人工项无容身之处，goal 追溯断裂，
  等于把"可验收"降级成"有测试"）。

### D10. CI 门禁：默认严格模式

- 任何机检失败 → 退出码非零（红）；
- 机检全过但存在未确认人工项 → 严格模式红（默认，报告要当合同证据），
  宽松模式黄（留给日常开发迭代，显式开关）。

### D11. 落地形态：新 crate `intent-lang-accept`；MVP 砍掉 LLM

workspace 新增 `crates/intent-lang-accept`，与 core 边界清晰：
core 出 testspec 和子句 ID，accept 消费它们。core 保持
"纯分析、无副作用、无 LLM、无进程执行"的干净定位不被污染。

CLI 新增子命令：

| 命令 | 职责 | 里程碑 |
|------|------|--------|
| `intent accept gen` | testspec + Z3 见证 + binding → 生成 pytest 文件（含人工项清单） | M-A1 |
| `intent accept run` | 执行 pytest → JUnit 归并 → JSON 验收工件 → 门禁退出码 | M-A1 |
| `intent bind` | LLM 起草 binding 草稿（人工确认后固化） | M-A2 |

MVP（M-A1）**全手动但全链路**：binding 格式没定型之前让 LLM 生成它是无的放矢。
先建确定性骨架，再往上装智能 —— 与 D4/D6 同一原则。

---

## 3. 架构总览

```
                    ┌─ 目标 A（已有）────────────────────────┐
.intent ──► parse ──► typeck ──► VCGen ──► Z3 一致性 ──► consistency_report
   │                    │
   │ 子句稳定 ID         │ W00xx: 不可执行子句 warning（D7）
   ▼                    ▼
testspec（子句 ID + 可执行性分类）
   │
   │        Z3 见证求解（D8: happy / 负面 / 边界模型）
   │              │
   ▼              ▼
┌─ 目标 B（本 RFC，intent-lang-accept）───────────────────────┐
│  binding 文件（.intent.bind.toml，人工审查、进 git，D3/D4）  │
│         │                                                   │
│         ▼                                                   │
│  intent accept gen ──► 生成 pytest 文件 + 人工项清单（D6）   │
│         │                                                   │
│         ▼                                                   │
│  intent accept run ──► pytest 执行 ──► JUnit XML             │
│         │                                                   │
│         ▼                                                   │
│  按子句 ID 归并 ──► intent.acceptance_report（JSON, D9）     │
│         │              │                                    │
│         ▼              ▼                                    │
│  CI 门禁退出码（D10）   visualizer HTML 渲染                  │
└──────────────────────────────────────────────────────────────┘
```

生成时（可含 LLM，仅 M-A2 起）：`intent bind` 起草 binding 草稿。
执行时（零 LLM，永远）：gen / run 全链路确定性。

---

## 4. 关键设计草案

以下为初稿方向，实现时允许调整细节，但不得违反第 2 节的决策。

### 4.1 子句稳定 ID

**修订（rfc-modeling-integrity D4）**：语言引入可选子句标签
（`ensure debit: ...`），ID 方案更新为**命名优先、序号兜底**：

- 有标签：`TransferSafe/debit`；无标签：`TransferSafe/ensure[0]`（声明顺序）；
- 已命名 ID 不受子句插入影响；未命名子句的序号漂移由"typecheck 对关键子句
  发命名 hint"来收敛，内容哈希方案废弃；
- ID 出现在：testspec 工件、生成的测试名（如
  `test_TransferSafe__debit__happy`）、acceptance_report、diff 失效分析。

### 4.2 binding 文件格式（`<name>.intent.bind.toml`）

```toml
[meta]
intent_file = "transfer.intent"
adapter = "python-pytest"
target = "bank_demo"            # 被测 Python 包/模块

[types.Account]
construct = "bank_demo.Account(owner={owner}, balance={balance}, active={active})"

[state."Account.balance"]
read = "{self}.balance"          # 后置状态观测方式；未声明 = 不可观测 → 人工项

[ops.TransferSafe]
call = "bank_demo.transfer({sender}, {receiver}, {amount})"
# 失败信号的形态（修订，rfc-modeling-integrity D3）：
# "拒绝 + 状态不变"的语义由需求层的 `else reject` 声明；
# binding 只回答"拒绝在这个实现里长什么样"
reject_signal = "raises"         # raises | returns_error:<pattern>
error_type = "bank_demo.TransferError"
```

设计约束：

- 声明式，不含控制流 —— binding 是数据不是程序，保证可审查、可 diff；
- 每个 `state.*` 条目回答"这个抽象状态怎么读"；ensure 引用了未声明的状态
  → 该子句自动归类为不可机检（D7）；
- `{placeholder}` 由 codegen 用 Z3 见证值填充（D8）。

### 4.3 `intent.acceptance_report` 工件（新增 kind）

```json
{
  "kind": "intent.acceptance_report",
  "file": "transfer.intent",
  "binding": "transfer.intent.bind.toml",
  "adapter": "python-pytest",
  "clauses": [
    { "id": "TransferSafe/ensure[0]", "status": "passed",  "scenarios": 3 },
    { "id": "TransferSafe/ensure[1]", "status": "failed",
      "detail": "expected receiver.balance == 150, got 149",
      "scenario": "happy(balance=100, amount=50)" },
    { "id": "safety/TotalPreserved",  "status": "sampled-passed", "samples": 12 },
    { "id": "TransferSafe/invariant[0]", "status": "manual-pending",
      "reason": "state 'ledger.entries' not observable in binding" }
  ],
  "goals": [
    { "name": "转账绝不能凭空创造或销毁资金",
      "machine": { "passed": 8, "failed": 1, "total": 10 },
      "manual":  { "confirmed": 0, "pending": 2 } }
  ],
  "summary": { "passed": 8, "failed": 1, "sampled": 1, "manual_pending": 2 },
  "gate": { "mode": "strict", "verdict": "fail" }
}
```

`status` 取值：`passed` | `failed` | `sampled-passed` | `sampled-failed` |
`manual-pending` | `manual-confirmed`。`sampled-*` 与 `manual-*` 永不计入
"证明性通过"，报告与渲染层必须显式区分（D7 诚实原则）。

退出码：`strict` 模式下 `failed > 0 || manual_pending > 0` → 非零；
`lenient` 模式下仅 `failed > 0` → 非零。

### 4.4 与现有工件协议的关系

- `testspec.draft` 升级：rows 增加 `clause_ids` 与 `executability`
  （`machine` | `manual` | `sampled`）字段 —— 新增字段，次要版本；
- artifacts.md 的"反勾结原则"保持成立且被强化：断言翻译由确定性 codegen
  完成，连"生成测试的 LLM"这个角色都被消除了（仅 binding 起草有 LLM，
  且产物经人工确认）。

---

## 5. 里程碑

### M-A1（MVP）：全手动、全链路竖切

1. 子句稳定 ID 落进 syntax/core（含可选子句标签，rfc-modeling-integrity D4；
   testspec、diff 同步升级）；
2. 不可执行子句的 typecheck warning + testspec 可执行性分类（D7；
   量词子句一律 manual-pending，`sampled` 推迟到 state RFC 后）；
3. 定义并解析 `.intent.bind.toml` 格式；
4. Z3 见证求解 API（复用 `smt.rs`，方向反转：求满足模型而非反例）——
   同一求解顺带修复 vacuity 缺陷（rfc-modeling-integrity D1，`V0020`）；
5. `crates/intent-lang-accept` + `intent accept gen` / `intent accept run`；
6. 样例：`examples/basics/transfer.intent` + 一个约 50 行的 Python demo
   银行实现 + **手写**的 binding 文件；
7. `intent.acceptance_report` 工件 + 严格/宽松门禁退出码；
8. `example` 块（rfc-modeling-integrity D5）作为第一批生成的 pytest 用例；
9. visualizer 增加验收报告渲染（可后置到 M-A1.5）。

语言层前置（先于或并行 M-A1，见 rfc-modeling-integrity 第 5 节）：
子句标签（D4）→ `modifies` frame 语义（D2）→ `require ... else reject`（D3）。

**MVP 的验收标准（自举）**：在 Python demo 里故意埋一个 bug
（对应现有 `TransferBuggy` 的"多扣 1"），`intent accept run` 的报告
必须把失败**归属到那条具体的 ensure 子句 ID 上**，且退出码非零。
做不到这一点，MVP 不算完成。

### M-A2：LLM 起草 binding

**落地形态修订**：不做 `intent bind` CLI 内置 LLM，改为 **Agent Skills**
（LLM 智能留在 agent 层，CLI 保持零 LLM 确定性——与 D4 同一原则）：

- `.agents/skills/write-intent`：自然语言需求 → `.intent`，
  用 `intent check` 反例闭环迭代到 verified；只写需求侧；
- `.agents/skills/implement-testspec`：读 testspec + 被测代码 →
  起草 `.intent.bind.toml`（人工确认后固化）→ `intent accept gen/run` →
  按子句状态归因，处理 manual 项；只做验收侧；
- 两个技能各自声明反勾结边界：必须在**独立会话**中使用，互相禁止
  修改对方的产物（LLM.md 反勾结原则）。

未落地部分：`intent diff` 联动（需求变更 → 标注失效验收项 →
增量 binding 更新草稿）留待后续。

### M-A3：第二适配器（Rust / cargo test），证明协议语言无关

- binding 格式与 codegen 模板抽象出 adapter trait；
- intent-lang 对自己的代码验收（自举演示）。

---

## 6. Non-goals

- **不做实现正确性证明**：验收 = 对具体执行的判定（含抽样），不是 Dafny 式
  程序验证。POSITIONING.md 的边界不变；
- **不做执行时 LLM**：任何"agent 现场跑验收"的形态永久排除在
  `intent accept` 语义之外（D4）；
- **coverage 语义不变**：仍是语法级沟通工具，不因验收管线的存在而被
  重新解释为证明；
- **第一版不做 property-based 测试**（hypothesis 为远期增强，D8）。

---

## 7. Open questions（实现时决断，不阻塞本 RFC）

1. ~~子句 ID 的序号漂移：纯序号 vs 内容哈希辅助（4.1）~~
   **已关闭（rfc-modeling-integrity D4）**：可选子句标签，命名优先、序号兜底；
2. 人工项的"确认"机制：签署文件（如 `acceptance-signoff.toml`）进 git，
   还是 CLI 交互式确认后写入报告？倾向前者（可审计）；
3. Z3 见证值对 `String` / `Seq` 等非数值类型的构造质量，以及模型多样性
   （避免每次都解出 `0` 这类退化值）——
   **大部分已关闭（rfc-modeling-integrity D5）**：`example` 块提供人挑的
   业务值作为 happy-path 首选数据，Z3 求解值只补边界和负面；
   剩余：边界/负面场景的非数值类型构造质量，实现时评估软约束方案；
4. binding 中 fixture 的副作用管理（数据库、全局状态）：MVP 用纯内存 demo
   回避，M-A2 前需要 setup/teardown 声明设计。
