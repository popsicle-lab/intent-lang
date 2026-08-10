# intent-lang 语法规范

> 本文档是 intent-lang 的权威语法参考。

---

## 1. 程序结构

```ebnf
program     ::= declaration*
declaration ::= import_decl | type_decl | enum_decl | function_decl
              | intent_decl | safety_decl | theorem_decl | axiom_decl
              | goal_decl | coverage_decl | example_decl
```

---

## 2. 类型系统

### 2.1 内置类型

| 类型 | 说明 | 示例 |
|------|------|------|
| `Int` | 任意精度整数 | `42`, `-1` |
| `Bool` | 布尔值 | `true`, `false` |
| `String` | 字符串 | `"hello"` |
| `Seq<T>` | 有序序列 | `Seq<Int>` |
| `Set<T>` | 无序集合 | `Set<String>` |

### 2.2 结构体

```intent
type Account {
  balance: Int
  owner: String
  active: Bool
}
```

```ebnf
type_decl   ::= "type" IDENT type_params? "{" field_list "}"
type_params ::= "<" IDENT ("," IDENT)* ">"
field_list  ::= (IDENT ":" type_expr)*
```

### 2.3 枚举

```intent
enum Role { Admin, Editor, Viewer }
```

```ebnf
enum_decl ::= annotation* "enum" IDENT "{" IDENT ("," IDENT)* "}"
```

标 `@lifecycle` 的枚举参与状态机结构检查，见 §6.7.2。

### 2.4 泛型

```intent
type Pair<A, B> {
  first: A
  second: B
}
```

---

## 3. 意图声明

核心构造。声明一个操作应满足的条件，不写实现。

```intent
intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  modifies sender.balance, receiver.balance               // frame（可省略）

  require positive: amount > 0 else reject                // 业务规则（违反 ⇒ 拒绝）
  require funds:    sender.balance >= amount else reject
  ensure debit:  sender.balance' == sender.balance - amount    // 后置条件
  ensure credit: receiver.balance' == receiver.balance + amount
  invariant no_overdraft: sender.balance' >= 0             // 不变量
}
```

```ebnf
intent_decl ::= annotation* "intent" IDENT "(" param_list ")" "{" intent_item* "}"
intent_item ::= modifies_clause | clause
modifies_clause ::= "modifies" ("*" | path ("," path)*)
clause      ::= clause_kw (IDENT ":")? expr ("else" "reject")?
clause_kw   ::= "require" | "ensure" | "invariant"
path        ::= IDENT ("." IDENT)*
```

### 语义

| 子句 | 含义 | 何时必须为真 |
|------|------|------------|
| `require P` | 前置条件（调用方契约，违反 = 未定义行为） | 调用前 |
| `require P else reject` | **业务规则**：违反 ⇒ 操作被观测地拒绝且**所有状态不变** | 调用前 |
| `ensure Q` | 后置条件 | 执行后 |
| `invariant I` | 不变量 | 执行前后都必须 |

**验证条件**：`(∧ require_i) ∧ (∧ ensure_k) ∧ (∧ invariant_j) ∧ frame → (∧ invariant_j')`
（ensure 定义后状态、被假设；invariant' 与 safety' 是证明目标。）

**不变量的证明目标形态**：`invariant` 意为"任何状态下都成立"，所以前置形态
被当作假设、**后置形态才是证明目标**。若整条表达式一个 prime 都没有，其中的
状态字段在用作目标时自动补 prime：`invariant a.balance >= 0` 的义务是
`a.balance' >= 0`。若表达式任何位置出现了 prime，说明作者在刻意关联前后态
（`a.balance' >= a.balance`），则原样使用——再补 prime 会把它压成恒真式。
裸标识符（标量入参 `amount`、枚举变体 `Active`）不是状态，永不补 prime。

**空洞检查（V0020）**：额外做一次 SAT 检查 —— 若 intent 自身子句不可满足
（如 `require amount > 0` 与 `require amount < 0` 并存），任何"验证通过"
都是空洞真，报 `self-contradictory` 错误而非 verified。

### 子句标签（D4）

`ensure debit: ...` 中的 `debit` 是可选标签，为子句提供稳定 ID
（`TransferSafe/debit`）；未命名子句用声明序号兜底（`TransferSafe/ensure[0]`）。
标签在 intent 内必须唯一（`E0006`）。ID 贯穿 testspec、验收报告与 diff。

### `modifies` 与 frame 语义（D2）

- 显式 `modifies a.b, c.d`：intent 只允许改这些状态路径；ensure 中 primed
  的路径必须 ⊆ modifies（`E0007`）；
- 省略：frame 自动推断为 ensure 中出现 primed 的路径集合
  （invariant 的 primed 是证明目标、不计入 frame）；
- **frame 之外的可观测状态默认不变**（`x' == x` 被注入假设，且成为
  验收测试的断言）；
- `modifies *`：显式放弃 frame 语义（弱规范，不推荐）。

### `else reject`（D3）

`require P else reject` 把"违反怎么办"从 binding 层收回到需求层：
违反 ⇒ 拒绝 + 状态不变。每条带标记的 require 会生成一条额外 VC
（`Intent/label (reject branch)`）：`¬P ∧ 其余 require ∧ 空 frame`
必须与 invariant/safety 相容；验收管线为它生成"期待拒绝 + 状态不变"
的负面测试。拒绝的具体形态（异常/错误码）由 binding 声明。

### Primed 变量

`x'` 表示变量执行后的新值。同时支持 `after(x)` 作为等价别名，方便 LLM 生成和阅读。

```intent
// 以下两种写法等价：
ensure sender.balance' == sender.balance - amount
ensure after(sender.balance) == sender.balance - amount
```

规则：

- 只能出现在 `ensure` 和 `invariant` 中
- 类型与 `x` 相同
- 支持嵌套字段：`account.balance'` 或 `after(account.balance)`
- `after(...)` 在 AST 层面脱糖为 primed 形式

---

## 4. 安全规则

全局不变量，自动附加到同作用域所有 intent 的验证条件中。

```intent
safety HomeSafety(home: Home) {
  invariant !home.occupied ==> home.frontDoor.locked
  invariant home.frontDoor.open ==> !home.frontDoor.locked
}
```

```ebnf
safety_decl ::= "safety" IDENT "(" param_list ")" "{" invariant_clause* "}"
```

### 作用范围按参数名绑定

safety 的参数不是全称量词，而是**按名字匹配**的自由符号：上例约束的是符号
`home`，只有同样声明了 `home: Home` 的 intent 才会被附加这条规则。参数对不上
的 intent 直接跳过——那些符号指向它够不到的状态，frame 也不会为其生成等式，
硬要它证明只会得到无意义的反例。

这带来一个已知漏网场景：若某 intent 用别的参数名操作同类型状态
（`intent Foo(h: Home)`），它逃出 `HomeSafety(home: Home)` 的管辖。**建议同一
类型的状态参数在全文件内统一命名。** 这是名字绑定的固有限制，将来若为 safety
引入真正的全称量化即可消除。

---

## 5. 定理

需要被 SMT solver 证明的性质。

```intent
theorem TransferPreservesTotal {
  forall s: Account, r: Account, a: Int,
    TransferSafe(s, r, a) ==>
      s.balance' + r.balance' == s.balance + r.balance
}
```

```ebnf
theorem_decl ::= "theorem" IDENT "{" expr "}"
```

---

## 6. 公理

向 SMT solver 注入领域知识。被无条件假设为真。

```intent
axiom temp_monotonic {
  forall t: Thermostat,
    t.mode == Heat && t.target > t.temperature ==>
      t.temperature' > t.temperature
}
```

```ebnf
axiom_decl ::= "axiom" IDENT "{" expr "}"
```

> ⚠️ 错误的公理会导致验证不可靠 (unsound)。公理必须经过领域专家审核。

---

## 6.5 业务目标 (Goal — RFC A1)

`goal` 块声明业务层意图——为什么这套规则存在。
与 `safety`/`intent` 同级，是一等语法节点（不是注释）。

```intent
goal "用户余额永远满足透支底线" {
  rationale: "防止账户被透支放任，符合监管要求"
  stakeholder: ["finance", "compliance"]
  measure: "审计日志中无 balance < -overdraftLimit 的快照"
  realized_by: [BalanceSafety, Transfer, Refund]
}
```

```ebnf
goal_decl  ::= "goal" STRING "{" goal_field+ "}"
goal_field ::= "rationale"   ":" STRING
             | "stakeholder" ":" "[" STRING ("," STRING)* "]"
             | "measure"     ":" STRING
             | "realized_by" ":" "[" IDENT  ("," IDENT)*  "]"
```

**类型检查**：`realized_by` 中的每个标识符必须是已声明的
`safety` / `intent` / `theorem` 名字；否则发出 `W0010`（warning，
不阻断 CI，但会出现在 `intent.consistency_report` 中）。

**用途**：`intent impact` 通过 `realized_by` 反查"修改某条 intent
影响哪些目标 / 哪些 stakeholder"，见 `docs/protocol/artifacts.md`。

---

## 6.6 完备性维度 (Coverage — RFC A3)

`coverage` 块声明应该被规则覆盖的领域维度笛卡尔积。

```intent
coverage "permission-matrix" {
  dimensions: {
    role: [Admin, Editor, Viewer, Guest]
    sensitivity: [Public, Internal, Confidential, Secret]
    user_state: [authenticated, banned]
  }
}
```

```ebnf
coverage_decl ::= "coverage" STRING "{" "dimensions" ":" "{" dim+ "}" "}"
dim           ::= IDENT ":" "[" expr ("," expr)* "]"
```

**类型检查**：维度值是 *opaque labels*（域内符号），
**故意不做** 类型检查。它们只用于 `intent coverage` 工具的语法级
见证扫描——在每条 safety/intent 子句的文本中查找字面值出现。

**为什么这样设计**：完备性是 *沟通工具*，不是证明工具——
让 reviewer 看到"哪些组合根本没人提及"已经有足够价值。
完整理由见 `docs/lang/DECISIONS.md` 决策 8。

---

## 6.7 时态标注 (As-is / To-be — RFC A2)

复用现有 `@annotation` 系统区分意图的生命周期：

```intent
@asis
intent LegacyV1(...) { ... }   // 老代码现在的实际行为

@tobe
intent NewV2(...) { ... }      // 新承诺的目标行为
```

| 标注 | 含义 | `intent check` 默认 |
|------|------|---------------------|
| `@asis` | 老代码的实然 | **跳过**（除非 `--include-asis`） |
| `@tobe` | 新承诺的应然 | 验证 |
| 无标注  | 当前规则 | 验证 |

迁移场景的典型用法：把老逻辑翻译成 `@asis` 一次性入库，
再为新承诺写 `@tobe`，让 review 同时看到"现状"与"目标"。

---

## 6.7.1 可视化注解 (@capability / @guardrail / @doc)

这几个注解**不参与 Z3 验证**，只驱动 `intent-lang-visualizer` 的分组与讲解。

```intent
@capability("自助售后闭环")
goal "客户能自助完成售后闭环" { realized_by: [CreateTicket, AssignTicket, ...] }

@guardrail("自助售后闭环")
goal "客户只能操作自己的工单" { ... }

@doc("客户建单主流程：校验订单归属与售后政策，通过则建单入队")
intent CreateTicket(c: Customer, t: Ticket, o: Order) { ... }
```

| 注解 | 作用于 | 语义 |
|------|--------|------|
| `@capability("组名")` | `goal` | 正向能力目标；位置参数是所属**主题组**，同组聚成一个 subgraph |
| `@guardrail("组名")` | `goal` | 护栏目标；同组内画边但不强制共享归属 |
| `@doc("一句话")` | `intent` / `goal` | 人类可读说明；渲染为 Goal Graph / State Machine 图下方的图例表，交互式 HTML 中还作节点悬浮提示 |

不写这些注解时可视化回退到平铺图 / 无图例，向后兼容。分组与图例的详细
渲染规则见 `tools/visualizer/GUIDE.md`。

---

## 6.7.2 生命周期标注 (@lifecycle — rfc-workflow-hardening D3)

`@lifecycle` 标在 `enum` 上，声明"这个枚举是一条生命周期"。只有被声明的枚举才
参与状态机结构检查（S0003–S0007）：

```intent
@lifecycle
enum RegistrationPhase { Entry, SiteResolved, Registered }

@lifecycle
enum SnQueryPhase { Idle, Querying, Answered }
```

- **可标多个**：每个声明的枚举独立推导一张状态机、独立跑一遍可达性，
  各自出一张图（`--all` 会额外导出 `statemachine-<enum>.mmd`）；
- **不声明就完全不检查**，且不报警。这是刻意的：转账、排序这类域本来就没有
  生命周期，"猜一个主导枚举"会把 `Priority` 这样的普通枚举当生命周期分析，
  产出凭空捏造的"状态不可达"。宁可漏报，不可假阳性；
- 可视化仍保留旧的启发式（猜主导枚举）以便老文件照样出图——**门槛**要求显式声明，
  **画图**不要求。

状态机由子句结构推导：`require x == Variant` 是源态，`ensure x' == Variant`
是次态；只有次态没有源态的 intent 构成 **creation 边**（`[*] --> 初始态`）。

| 码 | 含义 | 默认严重度 |
|----|------|-----------|
| `S0001` | intent/safety 未被任何 goal 的 `realized_by` 认领 | warning |
| `S0002` | intent 没有 `example` 块（`@asis` intent 豁免，见下） | warning |
| `S0003` | `@lifecycle` 枚举没有 creation 边，或没有驱动任何迁移 | warning |
| `S0004` | 存在 creation 边的前提下，某状态从 creation 不可达 | **error** |
| `S0005` | 没有终态（所有状态都有出边） | warning |
| `S0006` | 某些状态到不了任何终态（陷阱环） | warning |
| `S0007` | 一个 intent 无条件断言多个互斥次态（结构级 V0020） | **error** |
| `S0008` | 一条声明的生命周期有多于一个入口（多条 creation 边） | warning |

`intent check --strict` 把上表的 warning 全部升为 error。只有 S0004 与 S0007
默认即 error，因为只有它们是无歧义的缺陷：没有 creation 边可能是"创建发生在本文件
之外"，没有终态可能是"长生命周期实体（如 Active ↔ Frozen）"——把这些判成错误会逼
作者编造假的 Bootstrap 操作或假的终态，为迁就工具而污染需求。

`S0008` 值得单说：一条生命周期本该只有一个入口。若某个 intent 断言了 `phase' == X`
却把前提写成 Bool（`require ctx.devicesFound` 而不是 `require ctx.phase ==
DevicesResolved`），这条边就失去源态、被推导成第二条 creation 边——链条被切断，而
`S0004` 恰好因此失效（自带 creation 边的状态天然可达）。实测中它把建模成立的生命
周期（各 1 条入口）与破碎的（2 条和 3 条，其中一条直接进终态）完全分开。默认
warning 而非 error，是因为双入口确实可能合法（实体既可注册创建也可导入创建）。

`S0002` 不适用于 `@asis` intent。`@asis` 记录既有代码的实然行为，其权威示例是产线
数据与存量测试，由验收环节采集；在建模期逐条索要只会得到编造的数值，而编造比缺失
更坏。实测一个逆向建模的真实服务：29 个 intent 全部是 `@asis`、23 个没有 example，
这条检查因此淹没了真正能区分建模成立与否的信号。

---

## 6.8 示例块 (Example — rfc-modeling-integrity D5)

`example` 块用作者挑选的具体值锚定形式化规范（specification by example）。

```intent
example TransferSafe "工资转账" {
  given:  { sender.balance: 1000, receiver.balance: 50, amount: 300 }
  expect: { sender.balance': 700, receiver.balance': 350 }
}
```

```ebnf
example_decl ::= "example" IDENT STRING? "{" example_section* "}"
example_section ::= ("given" | "expect") ":" "{" binding ("," binding)* "}"
binding      ::= path "'"? ":" literal
```

三重角色：

1. **防形式化偏差**（唯一的机器检查手段）：`intent check` 用 Z3 把具体值
   代入每条子句，矛盾即报 `V0021` 并指出违反的具体子句 ——
   "公式自洽但不是作者想要的"从此可被抓住；
2. **验收种子数据**：`intent accept gen` 把 example 生成第一批 pytest
   用例（业务值比 Z3 求解的退化值质量高）；
3. **可读文档**：非程序员 stakeholder 能看懂具体数字。

规则：`given`/`expect` 的值必须是字面量（`E0009`）；`expect` 的路径应为
primed（`W0012`）；`expect` 可以只写部分后状态，未写的字段遵循 frame 语义。

整数字面量可为负：`b.velocityY: -164` 合法。词法层只产出非负数字，`-164` 由
解析器在前缀负号处折回字面量，因此它在所有位置都与 `164` 同等看待。这对重力、
温度、增量、欠款一类天然为负的量是必需的——曾因缺此支持而整类场景写不出 example。

---

## 6.9 验证结论的取值

`intent check` 对每条 VC 给出下列结论之一：

| 结论 | 含义 |
|------|------|
| `verified` | 目标在假设下成立 |
| `failed` | 不成立，附反例（一组使目标为假的取值） |
| `unknown` | Z3 超时或落在不可判定片段——**不是**通过 |
| `self-contradictory` | 子句本身不可满足，实现为空洞真（`V0020`） |
| `error` | 工具未能得出可信结论，拒绝作答 |

最后一项目前只有一个来源：发射的 SMT-LIB2 没有被 Z3 完整接受
（`Z3 rejected part of the generated SMT-LIB2 (N assertion(s) written, M accepted)`）。
Z3 会**丢弃**无法解析的断言并继续求解，剩下一个约束更弱的问题；求解器于是照常
给出 sat/unsat，而那个答案回答的不是提出的问题。检查器比对写出与收到的断言条数，
不一致即拒绝作答——宁可无结论，不可给错结论。

出现它说明 intent-lang 的编码器有缺陷，而非需求写错。用 `--show-smt` 看发射内容；
已知触发源见 §7。

---

## 7. 纯函数

辅助定义，无副作用。

```intent
function max(a: Int, b: Int) -> Int {
  if a >= b then a else b
}
```

```ebnf
function_decl ::= "function" IDENT "(" param_list ")" "->" type_expr "{" expr "}"
```

> **当前实现不验证 function。** 语法与类型检查接受它，但 VC 生成器不会把函数体
> 编码进 SMT——调用点会以未声明符号的形式发射，Z3 拒绝该断言。`intent check`
> 因此报 `Z3 rejected part of the generated SMT-LIB2`（见 §6.9），拒绝给出结论。
>
> 在实现之前，请把函数体**手工内联**到子句里。这条限制曾长期不可见：Z3 丢弃
> 无法解析的断言后继续求解，剩下一个约束更弱的问题，于是有证明目标时报「失败
> 且无反例」、无证明目标时报「已验证」——两种都是无依据的结论。

---

## 8. 模块系统

### 8.1 导入

intent-lang 有两种导入：**插件导入** 和 **文件导入**。

```intent
// 插件导入：加载类型 + safety + axiom + 函数
import smarthome
import finance.currency

// 文件导入：仅引入符号（类型、函数、intent 等），不注入 safety/axiom
import "./types/order.intent"
import "./common/helpers.intent"
```

```ebnf
import_decl  ::= "import" import_path
import_path  ::= module_path | STRING_LITERAL
module_path  ::= IDENT ("." IDENT)*
```

**两者的区别：**

| | 插件导入 (`import smarthome`) | 文件导入 (`import "./foo.intent"`) |
|---|---|---|
| 来源 | 已安装的插件包 | 项目内的 `.intent` 文件 |
| 类型/函数 | ✅ 引入 | ✅ 引入 |
| safety 规则 | ✅ 自动合并到验证条件 | ❌ 不合并（需在文件中显式声明） |
| axiom | ✅ 注入 SMT | ❌ 不注入 |
| 场景 | 领域基础设施（物理约束、行业规则） | 项目内代码组织 |

### 8.2 限定名

当多个导入引入同名类型时，使用限定名消歧：

```intent
import finance      // 定义了 Account
import user         // 也定义了 Account

// 用模块前缀消歧
intent Checkout(wallet: finance.Account, profile: user.Account) {
  require wallet.balance >= price
  require profile.authenticated
}
```

文件导入的限定名默认为文件名（不含扩展名），可用 `as` 自定义别名：

```intent
// 默认：前缀为文件名
import "./domains/order.intent"          // 前缀 order
import "./domains/payment.intent"        // 前缀 payment

// 两个文件名相同时，用 as 消歧
import "./domains/payment/types.intent" as payment
import "./domains/user/types.intent" as user

intent ProcessOrder(o: order.Order, wallet: payment.Account, profile: user.Account) {
  require p.amount == o.total
  require profile.authenticated
}
```

```ebnf
import_decl    ::= "import" import_path ("as" IDENT)?
import_path    ::= module_path | STRING_LITERAL
module_path    ::= IDENT ("." IDENT)*
type_expr      ::= qualified_type | IDENT | IDENT "<" type_expr ("," type_expr)* ">"
qualified_type ::= module_name "." IDENT
module_name    ::= IDENT
```

**规则：**

- 插件导入的限定名前缀为最后一段路径：`import finance.currency` → 前缀 `currency`
- 文件导入的限定名默认为文件名（不含 `.intent`），可用 `as` 覆盖
- 无冲突时，可直接使用类型名：`Account`
- 存在同名冲突时，必须使用限定名，否则报编译错误
- 限定名始终可用，即使无冲突

### 8.3 注解

```intent
@source("PRD-2024-Q1", section: "3.2.1")
@priority(P0)
intent PaymentSafe(...) { ... }
```

```ebnf
annotation  ::= "@" IDENT "(" annotation_args? ")"
```

---

## 9. 表达式

```ebnf
expr ::= literal | IDENT | IDENT "'"
       | "after" "(" expr ")"
       | expr "." IDENT | expr "." IDENT "'"
       | "!" expr | "-" expr
       | expr binop expr
       | "if" expr "then" expr "else" expr
       | "forall" typed_vars "," expr
       | "exists" typed_vars "," expr
       | IDENT "(" expr_list ")"
       | expr "[" expr "]"
       | "(" expr ")"

binop ::= "==" | "!=" | "<" | "<=" | ">" | ">="
        | "+" | "-" | "*" | "/" | "%"
        | "&&" | "||" | "==>"
```

### 优先级（低 → 高）

| 级别 | 运算符 | 结合性 |
|------|--------|--------|
| 1 | `==>` | 右 |
| 2 | `\|\|` | 左 |
| 3 | `&&` | 左 |
| 4 | `==`, `!=` | 无 |
| 5 | `<`, `<=`, `>`, `>=` | 无 |
| 6 | `+`, `-` | 左 |
| 7 | `*`, `/`, `%` | 左 |
| 8 | `!`, `-` (一元) | 前缀 |
| 9 | `.`, `'`, `[]` | 后缀 |

---

## 10. SMT 编码策略

### 类型映射

| intent-lang | SMT-LIB2 |
|-------------|----------|
| `Int` | `Int` |
| `Bool` | `Bool` |
| `String` | `String` |
| struct | `(declare-datatype ...)` |
| enum | `(declare-datatype ...)` |
| `Seq<T>` | `(Array Int T)` |

### Intent 验证编码（反证法）

```smt2
(assert R1)              ; require
(assert R2)
(assert E1)              ; ensure（定义后状态，被假设）
(assert E2)
(assert V1)              ; invariant (旧状态)
(assert F1)              ; frame: x' == x（frame 外状态，D2）
(assert (not (and V1_prime S1_prime)))  ; 否定 invariant' + safety'
(check-sat)
; unsat → 验证通过 | sat → 反例
```

**空洞检查（V0020，D1）**：上述结果为 unsat 时再做一次
`(assert R*) (assert E*) (assert V*) (assert F*) (check-sat)`——
若也是 unsat，说明子句集自相矛盾，报 `self-contradictory`。
sat 时得到的模型即 happy-path 见证值（D8，同一次求解两用）。

### Theorem 验证编码

```smt2
(assert (not <theorem_body>))
(check-sat)
```

---

## 11. 错误报告

采用 rustc 风格的诊断信息：

```
error[E0001]: type mismatch
  --> transfer.intent:12:11
   |
12 |   require amount > "zero"
   |                     ^^^^^^ expected Int, found String

error[V0001]: verification failed
  --> transfer.intent:49:3
   |
49 |   ensure sender.balance' == sender.balance - amount - 1
   |   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ cannot be verified
   |
   = counterexample:
       sender.balance = 100, amount = 10
     expected: 149 == 160 (off-by-one)
```

### 隐式安全规则展示

当 intent 验证时，作用域内**参数可匹配的** `safety` 规则（见 §4）被隐式合并
到验证条件中。为避免黑盒效果（LLM 和用户看不到实际验证了什么），报告中应展示
完整约束——包括哪些规则因参数对不上而未被附加：

```
info[V0010]: verification context
  --> smarthome.intent:25:1
   |
25 | intent SetTemperature(...) { ... }
   |
   = applied safety rules:
     - HomeSafety.invariant[1]: !home.occupied ==> home.frontDoor.locked
     - HomeSafety.invariant[2]: home.frontDoor.open ==> !home.frontDoor.locked
   = effective verification condition:
     (require_1 ∧ require_2 ∧ safety_1 ∧ safety_2) → (ensure_1 ∧ safety_1' ∧ safety_2')
```

这使得 LLM 可以理解失败原因，也让用户能审计验证过程。
