---
name: write-intent
description: 把自然语言需求形式化为 intent-lang 的 .intent 文件，并用 Z3 验证闭环迭代到 verified。当用户要"把需求写成 intent"、"形式化这段需求"、"建模需求"或给出一段业务规则要求落成 .intent 文件时使用。本技能只产出需求（.intent），不写 binding、不写测试代码——那是 implement-testspec 技能的职责，且必须在独立会话中进行（反勾结原则，见 docs/lang/LLM.md）。
---

# 需求 → .intent（含 Z3 验证闭环）

把用户的自然语言需求写成 `.intent` 文件，然后用 `intent check` 的反例反馈循环修正，直到全部 verified。

## 边界（反勾结原则）

本技能**只写需求，不写实现验收**：

- 禁止创建/修改 `.intent.bind.toml`、测试代码、被测实现；
- 需求定稿后告诉用户：在**新会话**中用 `implement-testspec` 技能做验收落地。
  同一个上下文既写需求又写测试会双向作弊（一致的 bug 互相掩护）。

## 语法模板

一个 intent 通常 5–15 行。完整语法见 `docs/lang/SPEC.md`，参考范例见
`examples/acceptance/transfer.intent`。

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
   - `V0020 SELF-CONTRADICTORY` → require/ensure 相互矛盾（空洞地"通过"被
     拦截了），检查符号方向、单位、primed 与否；
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
6. **不改用户语义**——形式化只是翻译。语义拿不准时列出两种公式让用户选，
   不要替用户决定。

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
- 用可视化工具自检：`intent-lang-visualizer <file> --check-states`
  （不可达/死状态/陷阱环会非零退出）。这是对"正向能力"的结构级验证——
  能力目标声称"客户能走完闭环"，可达性检查证明这条路径在状态机里真的存在。

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
