# RFC: 需求建模的忠实性与语言层补全（modeling integrity）

Status: Implemented (D1–D5, D7–D8 已落地；D6 `state` 块按计划推迟到语言 0.2 独立 RFC)
Created: 2026-07-15
Implemented: 2026-07-15 —— V0020/V0021 诊断、子句标签与稳定 ID、`modifies`
frame 语义、`require ... else reject`、`example` 块均已进入 syntax/core/CLI；
详见 CHANGELOG [Unreleased] 与 docs/lang/SPEC.md、DECISIONS.md 决策 11–15
Context: 在 rfc-executable-acceptance.md 定稿后，回头对需求建模层本身的
第二轮系统性拷问（grilling session），共 8 项决策逐一确认后收敛而成
Related: docs/rfc-executable-acceptance.md, docs/lang/SPEC.md,
docs/lang/DECISIONS.md, docs/lang/POSITIONING.md

---

## 0. 一句话主张

> 需求建模语言的最大信任危机不是"逻辑不自洽"，而是两种更隐蔽的失真：
> **工具说绿但其实没证明任何东西**（空洞验证），以及
> **式子自洽但不是作者的本意**（形式化偏差）。
> 本 RFC 补的是这两个洞，外加验收管线（目标 B）对语言层提出的表达力欠账。

---

## 1. Motivation

rfc-executable-acceptance.md 把项目目标定为"实现可对需求验收"（目标 B），
并明确目标 A（需求自洽）是前置门槛。本轮拷问回头审视这个门槛和语言本身，
发现的问题分三类：

1. **正确性缺陷**：目标 A 的核心承诺（"验证意图之间无矛盾"，DECISIONS 决策 5）
   存在空洞验证漏洞 —— 自相矛盾的需求显示 ✅ verified（见 D1，已实验证实）；
2. **表达力欠账**：frame 语义缺失、失败路径行为无法建模、子句不可命名、
   缺少具体例子锚点 —— 每一项都直接削弱验收管线的可信度或可读性；
3. **结构性缺失**：语言没有"系统状态/实体集合"概念，需求只能谈参数不能谈
   "系统里现存的东西"—— 认定为正确方向但深水区，排队处理。

---

## 2. 决策记录（8 项，已确认）

### D1. 空洞验证（vacuity）是 P0 正确性缺陷，修法：verified 结果加二次 SAT 防伪

**实验证据**（2026-07-15，`intent check` 实测）：

```intent
intent Contradictory(a: Account, amount: Int) {
  require amount > 0
  require amount < 0        // 与上一条矛盾
  ensure a.balance' == a.balance - amount
  invariant a.balance' >= 0
}
```

输出：`✅ intent Contradictory — verified`。两条互相矛盾的 ensure 同样 ✅。

**根因**：`vcgen.rs` 把 require + ensure + invariant(pre) 全部作为 assumes，
只证明 invariant'；assumes 不可满足时 `assumes → goals` 空洞为真，
Z3 返回 unsat、工具报 verified。需求一致性检查器把"需求自相矛盾"
这种最严重的病标成绿色，不可辩护。

**修法**：对 verified 的结果追加一次 Z3 调用，检查 assumes 的可满足性：

- assumes UNSAT → 新 error 诊断 `V0020: intent is self-contradictory`，CI 红；
- assumes SAT → 维持 verified，**且得到的模型直接复用为验收管线 D8 的
  happy-path 见证值**（一组同时满足 require ∧ ensure 的具体赋值）。

**红利**：修此 bug 与实现验收数据生成是同一次求解 —— 归入验收 RFC
M-A1 第 4 步（Z3 见证求解 API），不单独排期。

- 否决：每条 intent 无条件双倍求解（浪费 —— 失败的结果本来就会被人看，
  只有绿色需要防伪）；可选开关 `--check-vacuity`（默认配置下自相矛盾
  显示 ✅ 对一致性工具不可辩护）。

### D2. `modifies` 子句 + frame 语义：未提及的状态 = 必须不变

**问题**：现状下 ensure 没提到的状态语义是"无约束"。一个把
`sender.owner` 改成 `"attacker"` 的实现完全满足现在的 `TransferSafe`。
"操作不该有额外副作用"是需求方默认在心里、从不写出来的那类需求；
验收测试对"其他都没变"无据可断。

**设计**：

- intent 可显式声明 `modifies sender.balance, receiver.balance`；
- 不写时自动推断为"ensure/invariant 里出现过 primed 的字段"；
- 语义：**frame 之外的可观测状态在操作前后必须相等**；
- Z3 侧对 frame 外字段注入 `x' == x`（一致性检查同时变强，
  总额守恒类定理更易证）；
- 验收侧对 frame 外的可观测状态免费生成"未变"断言；
- typecheck：显式写了 modifies 时，primed 出现的字段必须 ⊆ modifies；
- 需要弱承诺（欠约定）时显式写更大的 modifies 范围退回弱语义。

**定位合规**：modifies 不是"怎么实现"，而是"操作被允许影响世界的范围"，
本身就是一条业务需求，在 POSITIONING 边界内。

**代价**：语义 breaking change（部分现有例子验证结果变严格），
必须进 CHANGELOG 主要条目。

- 否决：强默认无逃生口（合法的欠约定需求被逼成错误）；
  维持现状（把一类真实需求永久排除在表达力之外，且各适配器各拍一套语义漂移）。

### D3. `require ... else reject`：把失败路径行为收进需求层

**问题**：violate-require 场景在 testspec 里 expect 为
`"behavior unspecified — caller error"`；"余额不足时会发生什么"这条
业务需求被降级为 binding 层的实现细节（`require_violation = "raises"`），
负面验收测试测的是"适配器作者猜的行为"。

**设计**：

```intent
intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  require amount > 0                             // 调用方契约：违反 = 行为未定义
  require sender.balance >= amount else reject   // 业务规则：违反 = 必须拒绝
  ...
}
```

`reject` 的需求级语义固定两条：**操作被明确拒绝（可观测的失败信号）+
全部状态不变（空 frame）**。失败信号的具体形态（异常类型、错误码）留给 binding。

**语义收益**：显式化了 require 里一直混着的两种东西 ——
调用方契约（违反是调用者的 bug）与业务规则（违反是系统必须优雅处理的
正常业务场景）。这是需求建模表达力的实质提升，不只服务验收。

**下游**：testspec 中被标记的 violate 场景获得真断言（"被拒绝 ∧ 状态未变"）；
未标记的保持 unspecified，按验收 RFC D7 归入不生成/人工项。
Z3 可几乎免费验证 reject 分支与 safety 不变量相容（状态不变 ⇒ 不变量保持）。

- 否决：为失败路径单独写 intent（每条规则写两遍，require 与失败条款物理分离，
  diff/追溯断裂）；维持现状（负面验收永远不可信）。

### D4. 可选子句标签：稳定 ID 的语言级解法

```intent
intent TransferSafe(...) {
  ensure debit:  sender.balance' == sender.balance - amount
  ensure credit: receiver.balance' == receiver.balance + amount
}
```

- ID 规则：有标签 → `TransferSafe/debit`；无标签 → 退回序号
  `TransferSafe/ensure[0]`；
- 语法：`IDENT ":"` 前瞻无歧义（冒号不在表达式文法中）；
- typecheck：同 intent 内标签唯一；对"被 goal 引用 / 标了 else reject /
  参与验收生成"的子句发命名 hint（不强制）；
- 收益：命名子句让 explain、验收报告、counterexample 三处输出从
  "第 N 条"升级为业务词汇（debit / credit / no-overdraft），
  对非程序员验收方是质变；中间插入子句不再引起已命名 ID 漂移。

**关闭验收 RFC open question 1**：命名优先、序号兜底，内容哈希方案废弃。

- 否决：纯工具层序号+哈希（可读性和漂移只是缓解，子句小改时哈希匹配
  必然出现灰区）；强制标签（抬高草稿门槛，与"LLM 起草、人来审"工作流冲突）。

### D5. `example` 块：specification by example 作为一等语法

```intent
example TransferSafe "工资转账" {
  given:  { sender.balance: 100, receiver.balance: 50, amount: 30 }
  expect: { sender.balance': 70, receiver.balance': 80 }
}
```

三重角色：

1. **防形式化偏差**：`intent check` 时 Z3 代入具体值验证 example 满足全部子句。
   作者写的式子与他举的例子打架 → 当场报错。这是唯一能机检
   "式子符合本意"的手段 —— 子句自洽（Z3 可查）≠ 子句表达了本意（Z3 不可查），
   example 是钉住本意的具体锚点；
2. **验收种子数据**：人挑的值天然有业务意义，作为 happy-path 首选数据；
   Z3 求解值只补边界和负面 —— 验收 RFC open question 3 的退化值问题大部分关闭；
3. **非程序员可读文档**：需求方看不懂 ∀，看得懂"100 转 30 剩 70"。

设计细节：`expect` 允许只写部分字段（欠约定友好），未写字段按 D2 的
frame 语义处理。纯增量语法，不动现有语义。M-A1 中 example 直接成为
第一批生成的 pytest 用例。

- 否决：例子留在 testspec/binding 工具层（不参与 `intent check`，
  防偏差角色消失）；只当文档注释（零机检价值）。

### D6. `state` 块（系统状态/实体集合）：正确方向，立独立 RFC，目标语言 0.2

**问题**：语言里没有"系统里现存的实体"概念 —— `forall a: Account`
量化的是类型全集（数学全体可能值），不是"系统里现在的账户们"。
safety 号称约束全局，"全局"在语言里无处安放；验收 RFC D7 的
"对当前可枚举实例抽样"语义踩在空气上。

**方向**（细节留给独立 RFC）：`state Bank { accounts: Set<Account> }`，
safety 量词绑定到状态集合（`forall a in bank.accounts`）。

**为什么不是现在**：深水区 —— VCGen/SMT 重编码（集合成员量化落入更难的
可判定性片段，unknown 会变多）、intent 语义模型换代（参数化断言 → 状态转换）、
modifies 语义从字段级升级、theorem 的 struct 量词债必须先还、
全部例子文档重写。这是语言 0.2 级别的版本事件，仓促塞进当前盘子
会把 vcgen/smt 拖进泥潭。

### D7. M-A1 与 state 的顺序：M-A1 绕开量词，不建过渡机制

- M-A1 样例（transfer + Python demo）不含量词，MVP 范围内量词子句
  一律按验收 RFC D7 归入人工项 —— 这是诚实且已设计好的降级路径；
- **不建**过渡性 binding 枚举器（为 MVP 建一个注定被 state 替换的机制是浪费，
  且会用兼容性绑住 state RFC 的设计自由度）；
- 量词抽样将来直接建在 state 语义上，一步到位。

**对验收 RFC D7 的修订**：抽样检查（`sampled` 状态）的启用推迟到
state RFC 落地后；在此之前量词子句全部为 `manual-pending`。

### D8. 时序与单位：双双 non-goals

- **时序**（"先认证后操作"）：TLA+ 领地，POSITIONING 已拒绝协议/状态机建模。
  state 块落地后，一部分时序需求可用状态不变量改写
  （"未认证的会话不能出现在已授权集合里"）；硬时序需求真出现时单独评估。
  否决 `requires_prior` 类轻量语法：看似轻，实则把语言拖进执行轨迹语义，
  Z3 编码复杂度与 state 块同量级却没有其通用回报；
- **单位**（`balance: Int` 是分还是元）：走插件路线
  （`import finance.currency` 提供 `Money` 类型），并作为插件运行时的
  **第一个验收用例**（顺手还掉"import 只解析不加载"的债）。
  诚实记录：插件运行时目前只有设计（docs/architecture/PLUGINS.md），
  此路线的真实排期依赖插件系统落地。
  否决轻量 newtype：与插件方案功能重叠，将来两套机制打架。

---

## 3. 复杂度预算声明

本轮语言面新增五个构造（vacuity 检查、modifies、else reject、子句标签、
example）已达上限。**在 state RFC 落地之前，不再接受新语法。**
后续任何语言扩展提案必须先回答："能否用现有构造 + 工具层解决？"

---

## 4. 与验收 RFC（rfc-executable-acceptance.md）的关系

| 本 RFC 决策 | 对验收 RFC 的影响 |
|-------------|-------------------|
| D1 vacuity | 并入 M-A1 第 4 步（同一次 Z3 求解），happy-path 见证值来源 |
| D2 modifies | 语言层前置，建议先于或并行 M-A1；验收获得"未变"断言 |
| D3 else reject | 语言层前置；binding 的 `require_violation` 降级为"失败信号形态"，语义由需求层定 |
| D4 子句标签 | 关闭 open question 1；4.1 的 ID 方案更新为"命名优先、序号兜底" |
| D5 example | 进 M-A1 实现范围（第一批生成的 pytest 用例）；大部分关闭 open question 3 |
| D6/D7 state | D7 的 `sampled` 语义推迟到 state 落地后启用；M-A1 量词全人工项 |
| D8 边界 | 无直接影响；单位用例挂到插件运行时排期 |

---

## 5. 实施顺序建议

1. **D1（vacuity）**：独立小改动，任何时候可做，最高优先级（正确性缺陷）；
2. **D4（子句标签）→ D2（modifies）→ D3（else reject）**：三者都动
   syntax + typeck + vcgen，建议按此序一个 PR 一个，D4 最简单先趟路；
3. **D5（example）**：依赖 D2 的 frame 语义（expect 部分字段的缺省解释），排最后；
4. **state RFC**：与上述并行起草，落地即语言 0.2；
5. 与验收 RFC M-A1 的合流点：D1/D4/D5 完成后，M-A1 全部前置就绪。

---

## 6. Open questions（实现时决断，不阻塞本 RFC）

1. `modifies` 推断在嵌套字段/索引表达式（`accounts[i].balance'`）上的
   粒度界定；
2. `else reject` 的失败信号在 acceptance_report 中是否需要独立 status
   （如 `rejected-as-expected`），还是并入 passed；
3. `example` 的 given 是否允许省略 require 已唯一确定的字段
   （推导 vs 显式，倾向显式 —— example 的价值就在于全部具体）；
4. 子句标签是否进入 SMT 输出的注释（便于从 Z3 反例直接对回业务词汇，倾向进）。
