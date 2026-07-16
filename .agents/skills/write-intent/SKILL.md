---
name: write-intent
description: 把自然语言需求形式化为 intent-lang 的 .intent 文件，并用 Z3 验证闭环迭代到 verified。当用户要"把需求写成 intent"、"形式化这段需求"、"建模需求"或给出一段业务规则要求落成 .intent 文件时使用。本技能只产出需求（.intent），不写 binding、不写测试代码——那是 implement-testspec 技能的职责，且必须在独立会话中进行（反勾结原则）。
---

# 需求 → .intent（含 Z3 验证闭环）

把用户的自然语言需求写成 `.intent` 文件，然后用 `intent check` 的反例反馈循环修正，直到全部 verified。

> **工具前置**：本技能只依赖编译好的 `intent` 命令行（用到 `intent check`、
> `intent testspec`）。状态机图 / 业务流程图与 `--check-states` 结构自检来自另一个
> **可选**二进制 `intent-lang-visualizer`——没有它照样能完成需求建模与 Z3 验证，
> 只是少了图形化自检。本文档语法部分自足，无需任何仓库内文件。

## 边界（反勾结原则）

本技能**只写需求，不写实现验收**：

- 禁止创建/修改 `.intent.bind.toml`、测试代码、被测实现；
- 需求定稿后告诉用户：在**新会话**中用 `implement-testspec` 技能做验收落地。
  同一个上下文既写需求又写测试会双向作弊（一致的 bug 互相掩护）。

## 语法模板

一个 intent 通常 5–15 行。下面是一个完整可跑的转账范例（复制即可 `intent check`）；
更全的声明形式见后面的「语法速查」，两者合起来即本语言的常用全集。

```intent
type Account {
  owner: String
  balance: Int
  active: Bool
}

goal "转账绝不能凭空创造或销毁资金" {
  rationale: "资金守恒是账务系统的底线"
  stakeholder: ["finance"]
  measure: "所有转账操作满足借贷平衡"
  realized_by: [TransferSafe]
}

intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  modifies sender.balance, receiver.balance

  require positive: amount > 0 else reject
  require funds:    sender.balance >= amount else reject

  ensure debit:  sender.balance' == sender.balance - amount
  ensure credit: receiver.balance' == receiver.balance + amount

  invariant no_overdraft: sender.balance' >= 0
}

example TransferSafe "工资转账" {
  given:  { sender.balance: 1000, receiver.balance: 50, amount: 300 }
  expect: { sender.balance': 700, receiver.balance': 350 }
}
```

要点：`x'`（primed）表示操作后的值；`&&` `||` `!` `==>` 为逻辑连接词；
`forall x: T, P(x)` 为量词（能不用就不用，见下面质量规则 5）。

## 语法速查（自足，无需外部文档）

内置类型：`Int`（任意精度整数）、`Bool`、`String`、`Seq<T>`、`Set<T>`。

声明形式（一个 `.intent` 文件由这些顶层声明组成）：

```intent
enum Status { Draft, Open, Done }               // 枚举
type Ticket { id: Int  status: Status }          // 结构体（字段用换行或空格分隔）
function max(a: Int, b: Int) -> Int {            // 纯函数（无副作用，可在子句里调用）
  if a >= b then a else b
}

intent Op(t: Ticket) {                           // 操作 = 核心构造
  modifies t.status                              // frame：只允许改这些路径；省略则从 primed ensure 推断；modifies * 放弃 frame
  require r: <expr> else reject                  // 前置业务规则：违反 ⇒ 观测地拒绝且所有状态不变
  ensure  e: <expr>                              // 后置条件（用 primed 描述新值）
  invariant i: <expr>                            // 执行前后都必须成立
}

safety Name(x: T) { invariant <expr> }           // 全局不变量，自动并入同作用域所有 intent 的 VC
theorem Name { forall x: T, <expr> }             // 待 SMT 证明的性质
axiom Name { forall x: T, <expr> }               // 无条件假设的领域知识（慎用：错误公理会使验证 unsound）
goal "一句话目标" {                               // 业务目标（为什么存在这套规则）
  rationale: "..."  stakeholder: ["..."]  measure: "..."  realized_by: [Op]
}
coverage "name" { dimensions: { d1: [a, b]  d2: [x, y] } }   // 应覆盖的维度笛卡尔积
example Op "场景名" { given: { t.status: Open }  expect: { t.status': Done } }
```

表达式与运算符：

- primed 新值：`x'` 或等价的 `after(x)`（仅可出现在 `ensure` / `invariant`）；
- 逻辑：`!`（非）、`&&`、`||`、`==>`（蕴含，右结合）；比较：`== != < <= > >=`；算术：`+ - * / %`；
- 条件表达式：`if C then A else B`；量词：`forall v: T, P` / `exists v: T, P`；
- 字段/下标：`a.b`、`a.b'`、`s[i]`；多导入同名类型用限定名 `module.Type` 消歧。

注解（除 `@tobe`/`@asis` 外均不参与 Z3，仅供工具/可视化）：

- `@tobe` 新承诺的应然（默认验证）/ `@asis` 老代码的实然（`intent check` 默认跳过）；
- `@capability("组名")` / `@guardrail("组名")` 标 `goal` 的类型与主题组；
- `@doc("一句话")` 给 `intent` / `goal` 挂人类可读说明。

## 工作流

1. **澄清需求**：识别实体（type）、业务目标（goal）、操作（intent）。
   需求含糊时先问用户，不要脑补业务规则。
2. **起草**：每个操作一个 intent；错误路径用 `require ... else reject`
   （语义 = 违反则拒绝且全部状态不变）；写清 `modifies`。
3. **验证闭环**：

```bash
intent check <file>.intent
```

   - `violated` + 反例（`variable = value`）→ 反例即缺失的前置条件或写错的
     后置条件，据此修正后重跑；
   - `V0020 SELF-CONTRADICTORY` → require/ensure 相互矛盾。两种可能：
     (a) **你译错了**（符号方向、单位、primed 与否）→ 修正；
     (b) **需求原文本身冲突**，你如实翻译了两条并列结论 → **别修**，这正是
     该暴露的问题，交 stakeholder 定夺后再删多余子句（见质量规则 6）；
   - `V0021` example 矛盾 → 例子与子句代入不一致，是形式化偏差的信号：
     先怀疑子句写错，而不是改例子迁就公式；
   - `W0011` → 量词子句不可机器执行，见质量规则 5；
   - 循环直到全部 verified 且退出码 0。
4. **自检产出**：`intent testspec <file>.intent` 预览下游会生成哪些场景，
   确认 happy path 和每条 require 的违反路径都符合业务预期。

## 质量规则

1. **每条子句都加标签**（`ensure debit: ...`）——标签是验收报告、diff
   失效分析的稳定 ID，未命名子句会有序号漂移问题；
2. **每个 intent 声明 `modifies`**——未列出的状态自动生成 frame 等式
   （证明其不变）；确实什么都可能改才用 `modifies *`；
3. **每条 require 考虑 `else reject`**——不加则违反时行为未定义，下游只能
   出人工测试项；
4. **至少写一个 `example` 块**——用户挑的业务值既防形式化偏差（Z3 代入
   校验），又是生成的第一批 pytest 数据。数值应向用户要真实业务值；
5. **优先可机检子句**——含 `forall`/`exists` 的子句静态分类为 manual，
   验收时进人工清单。能用有限参数表达就不用量词；
6. **只翻译，不自修复**——形式化只是翻译需求原文，**不是**修 bug。当文档
   对同一操作给出并列/冲突的结论（例：§6 说「无法处理关单→已关闭」、§10 说
   同一操作「→异常关单」），**如实把两条结论都写进同一个 intent**（两条 `ensure`
   并列），让 `intent check` 用 `V0020 SELF-CONTRADICTORY` / `V0021` 把矛盾报出来。
   严禁在建模时私自挑一方"顺手修好"——那会把需求缺陷藏进代码、蒙混过验证。
   矛盾定位后交 stakeholder 拍板，再删掉不要的那条子句。语义拿不准同理：列出
   两种公式让用户选，不替用户决定。（可选）若装了 `intent-lang-visualizer`，其
   `--check-states` 会用同样的结构信号把冲突在状态机图 / 流程图里标红，与 Z3 一致。

## 反模式（踩过的坑）

### 1. 授权别写成全局 `safety` 不变量

想表达"客户只能操作自己的工单"时，**不要**写：

```intent
safety CustomerOwnsTicket(c: Customer, t: Ticket) { invariant t.customerId == c.id }
```

这会被 Z3 读成"任意客户 × 任意工单都同属一人"，恒假，触发
`V0020 SELF-CONTRADICTORY`。授权是**每次操作的前置拒绝条件**，写在 intent 里：

```intent
intent CancelTicket(c: Customer, t: Ticket) {
  require owner: t.customerId == c.id else reject
  ...
}
```

判据：约束是"这次操作允不允许"→ 用 `require ... else reject`；是"任何状态下都成立的
数据关系"（如 `balance >= 0`）→ 才用 `safety` / `invariant`。

### 2. 状态机用 `require 当前态 → ensure 次态` 惯用法

状态流转靠一对子句表达，工具能据此自动还原状态机图并做可达性检查：

```intent
intent ResolveTicket(t: Ticket) {
  require in_progress: t.status == InProgress else reject   // 源态
  ensure  resolved:    t.status' == Resolved                // 次态
}
```

- 每个非初始状态都要有 intent 能进入它，否则是**死状态**；
- 每个状态都要能走到终态，否则是**陷阱环**；
- （可选，需 `intent-lang-visualizer`）用它自检：`intent-lang-visualizer <file> --check-states`
  （不可达/死状态/陷阱环会非零退出）。这是对"正向能力"的结构级验证——
  能力目标声称"客户能走完闭环"，可达性检查证明这条路径在状态机里真的存在。
  没有该二进制时，靠人工核对每个状态"进得来、走得到终态"即可。

### 3. goal 用 `@capability("组名")`/`@guardrail("组名")` 成对标注，止于能力级

只写护栏（"不许越权""状态不许非法跳转"）会让需求看起来全是禁令，读不出系统
**要成就什么**。每个主题成对写一个能力目标 + 若干护栏目标，并用注解标出类型
与所属主题组：

```intent
@capability("自助售后闭环")
goal "客户能自助完成售后闭环" {
  measure: "可从 CreateTicket 经受理、处理走到 CustomerConfirmResolved，无需人工"
  realized_by: [CreateTicket, AssignTicket, ResolveTicket, CustomerConfirmResolved]
}

@guardrail("自助售后闭环")
goal "客户只能操作自己的工单" { ... }
```

- **注解名 = 类型**（`capability` 正向 / `guardrail` 护栏），**位置参数 = 主题组名**。
  不要靠 goal 名里的文字（如 `[能力]`）区分——那是字符串嗅探，脆且机器读不准；
- 能力目标的 `realized_by` 列出**打通这条链路的 intent 序列**，其 `measure`
  是能力级、可结构验证的（"这条路径存在/走得通"），配合规则 2 的 `--check-states`；
- 同 `group` 的 goal 及其 realizer 会在 goal graph 里聚成一个 subgraph；被多个
  能力组共享的 intent 进"跨主题共享"块，没被任何 goal 认领的进"未被 goal 认领"块
  （后者是覆盖缺口信号）。不写注解的文件回退到平铺图，向后兼容；
- **止于能力级**：不写 "满意度提升 20%""降低客服成本" 等业务价值/ROI 指标——
  那属于 PRD，`.intent` 只承诺可形式化、可验证的东西；
- 语义时序型活性（"投诉最终一定被处理"）超出当前 SMT 能力，不要用 goal 假装
  能验证它，需要时在 PRD 里记为人工跟踪项。

### 4. 每个 intent（及 goal）加 `@doc("一句话")` 给可视化用

intent 名往往是驼峰缩写（如 `CreateTicketSoftReview`），单看图读不出它是什么。
用 `@doc` 挂一句人话说明，可视化会把它渲染成图例表 + 悬浮提示：

```intent
@tobe
@doc("仅退款的边界情形：不硬拒绝，允许建单但打上『需人工审核』标记，交人工判定")
intent CreateTicketSoftReview(c: Customer, t: Ticket, o: Order) { ... }
```

- `@doc` 是**非验证的补充散文**，不影响 Z3——纯给人看，别往里塞可验证语义；
- Goal Graph 与 State Machine 图下方会自动出「操作说明」图例表，交互式 HTML 里
  节点还带悬浮提示；不写则图例不出现，向后兼容；
- 一行讲清"这个操作在业务上做什么/什么边界情形"，不要复述子句。
