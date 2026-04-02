# 设计决策记录：Intent 理解方式与验证层次

本文档记录了 intent-lang 设计过程中对两个核心概念的分析与选择。

---

## 1. "Intent" 的三种理解方式

### 方向 A：自然语言意图 → 形式化表达式

用户用自然语言描述意图，系统翻译为形式化逻辑。

```
输入: "确保用户余额不低于转账金额"
输出: ∀ transfer: transfer.amount ≤ user.balance
```

| 维度 | 评估 |
|------|------|
| 易用性 | ⭐⭐⭐⭐⭐ 最自然 |
| 确定性 | ⭐ 自然语言歧义大 |
| 可验证性 | ⭐⭐ 翻译正确性无法保证 |
| 适合场景 | 辅助工具、原型探索 |

**问题**：NLP → 逻辑的映射不确定，与"formally provable"的确定性目标矛盾。LLM 可能对同一句话生成不同的形式化结果。

### 方向 B：意图声明式语法

语言本身围绕"我想要什么"设计，语法结构化、无歧义。

```intent
intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  require sender.balance >= amount
  ensure sender.balance' == sender.balance - amount
}
```

| 维度 | 评估 |
|------|------|
| 易用性 | ⭐⭐⭐ 需要学习语法 |
| 确定性 | ⭐⭐⭐⭐⭐ 完全确定 |
| 可验证性 | ⭐⭐⭐⭐⭐ 直接编码为 SMT |
| 适合场景 | 语言核心 |

**优势**：成熟路线（类似 Dafny/TLA+ 的 pre/post-condition），工程上完全可行。

### 方向 C：混合方式 ✅ 我们的选择

核心是方向 B（结构化声明），同时提供 LLM 辅助层帮用户从自然语言生成初始代码。

```
自然语言 ──LLM──→ intent-lang 代码 ──SMT──→ 验证结果
                  ↑                         |
                  └─── 用户审查/修改 ←───────┘
```

| 维度 | 评估 |
|------|------|
| 易用性 | ⭐⭐⭐⭐⭐ LLM 降低门槛 |
| 确定性 | ⭐⭐⭐⭐⭐ 最终验证走形式化路径 |
| 可验证性 | ⭐⭐⭐⭐⭐ LLM 生成的代码必须通过 SMT 验证 |
| 适合场景 | 生产级语言 |

**关键设计**：LLM 只是"草稿生成器"，不影响验证的安全性。即使 LLM 生成了错误代码，SMT solver 会拒绝它。

### 三种方式对比总结

```
确定性/安全性 ──────────────────────────→
高                                      低

  方向 B              方向 C              方向 A
  纯声明式             混合式              纯自然语言
  ┌──────┐          ┌──────┐          ┌──────┐
  │结构化  │          │结构化  │          │自然语言│
  │语法    │          │+ LLM  │          │ + NLP │
  └──────┘          └──────┘          └──────┘
  学习成本高          平衡点 ✅           歧义风险大

低                                      高
←────────────────────────── 易用性/门槛
```

---

## 2. "Formally Provable" 的四个层次

### L1: 类型检查

通过类型系统在编译期捕获错误。

| 代表 | TypeScript, Rust, Haskell |
|------|--------------------------|
| 保证 | 类型安全（不会把 String 当 Int 用） |
| 自动化 | ⭐⭐⭐⭐⭐ 全自动 |
| 保证强度 | ⭐⭐ 只能防类型错误 |
| 实现复杂度 | ⭐⭐ |

```typescript
// TypeScript — L1 能防止这种错误
function transfer(amount: number): void { ... }
transfer("hello") // ❌ 编译错误
transfer(-100)     // ✅ 编译通过（但逻辑上可能是 bug！）
```

**局限**：`transfer(-100)` 类型正确但逻辑错误，L1 无法检测。

### L2: 约束求解 / SMT ✅ 我们的选择

将验证条件编码为 SMT 问题，由求解器自动判定。

| 代表 | Dafny, Liquid Haskell, F*, **intent-lang** |
|------|-------------------------------------------|
| 保证 | 前置/后置条件、不变量、自定义属性 |
| 自动化 | ⭐⭐⭐⭐ 大部分自动，偶尔需要提示 |
| 保证强度 | ⭐⭐⭐⭐ 无界验证（不限状态空间） |
| 实现复杂度 | ⭐⭐⭐ |

```intent
// intent-lang — L2 能防止逻辑错误
intent TransferSafe(sender: Account, amount: Int) {
  require amount > 0              // SMT 自动验证
  require sender.balance >= amount
  ensure sender.balance' >= 0
}
// transfer(-100) → ❌ 违反 require amount > 0，SMT 给出反例
```

**优势**：用户不写证明，solver 自动求解。
**局限**：复杂量词嵌套可能 timeout（Z3 返回 `unknown`）。

### L3: 模型检查

穷举有限状态空间，检查所有可达状态是否满足性质。

| 代表 | TLA+ (TLC), Alloy, SPIN |
|------|-------------------------|
| 保证 | 在有界状态空间内的完全验证 |
| 自动化 | ⭐⭐⭐⭐⭐ 全自动 |
| 保证强度 | ⭐⭐⭐ 有界（只验证探索过的状态） |
| 实现复杂度 | ⭐⭐⭐ |

```tla
\* TLA+ — L3 穷举所有状态
Transfer(sender, receiver, amount) ==
    /\ amount > 0
    /\ balances[sender] >= amount
    /\ balances' = [balances EXCEPT
        ![sender] = balances[sender] - amount,
        ![receiver] = balances[receiver] + amount]

\* TLC 会尝试所有 sender × receiver × amount 的组合
\* 但只能在有限范围内（比如 balance 0~100, amount 1~50）
```

**优势**：全自动 + 生成执行 trace（便于理解系统行为）。
**局限**：状态爆炸——变量多或值域大时不可行。

### L4: 交互式定理证明

人机协作，用户编写证明策略，系统检查每一步。

| 代表 | Coq, Lean 4, Agda, Isabelle |
|------|-----------------------------|
| 保证 | 数学级别的绝对正确性 |
| 自动化 | ⭐⭐ 需要大量人工引导 |
| 保证强度 | ⭐⭐⭐⭐⭐ 最强（完全证明） |
| 实现复杂度 | ⭐⭐⭐⭐⭐ |

```lean
-- Lean 4 — L4 需要手写证明
theorem transfer_correct (sender receiver : Account) (amount : Int)
    (h : transferPre sender receiver amount) :
    let (s', r') := transfer sender receiver amount
    transferPost sender receiver s' r' amount := by
  unfold transferPre at h
  unfold transfer transferPost
  simp
  omega  -- 调用自动算术求解器
```

**优势**：保证最强，可以证明任意复杂的性质。
**局限**：学习曲线陡峭，证明编写耗时，需要类型论/逻辑学背景。

### 四个层次对比总结

```
保证强度 ──────────────────────────────→

  L1              L2              L3              L4
  类型检查         SMT求解          模型检查         定理证明
  ┌──────┐      ┌──────┐      ┌──────┐      ┌──────┐
  │TS/Rust│      │Dafny  │      │TLA+   │      │Lean  │
  │       │      │intent │      │Alloy  │      │Coq   │
  └──────┘      └──────┘      └──────┘      └──────┘
  只防类型错     自动验证逻辑    有界穷举        完全证明
  全自动         大部分自动      全自动          手写证明
  复杂度低        复杂度中       复杂度中         复杂度高

←──────────────────────────────── 自动化程度
高                                              低
```

### 为什么选择 L2？

| 考量 | L2 的优势 |
|------|----------|
| **用户体验** | 不需要学证明策略（vs L4），不需要限定状态空间（vs L3） |
| **保证强度** | 无界验证，比 L3 更强；对大多数实际场景已足够 |
| **与 intent 理念契合** | 用户只声明条件，验证全自动——和"只说 What 不说 How"一致 |
| **工程可行性** | Z3 成熟稳定，Rust bindings 完善，SMT-LIB2 标准通用 |
| **可退化** | 简单场景等价于 L1（类型检查）；必要时可引入 L3（有界检查）作为补充 |

### 未来演进路径

```
当前: L1 (类型检查) + L2 (SMT 自动验证)
              │
              ▼
Phase 2: + L3 模型检查（可选模式：intent check --bounded 100）
         用于并发/分布式场景，穷举有限状态
              │
              ▼
Phase 3: + L4 辅助（可选：对 SMT unknown 的场景提供 proof hint）
         用户可以给 Z3 提示，类似 Dafny 的 assert/calc 块
```

---

## 3. 决策矩阵

综合评估我们的最终选择：

| 维度 | 选择 | 理由 |
|------|------|------|
| Intent 表达 | **方向 C**（混合） | 结构化保证安全性 + LLM 降低门槛 |
| 验证层次 | **L2**（SMT） | 自动化高 + 保证强 + 工程可行 |
| 实现语言 | **Rust** | 性能 + WASM + Z3 bindings |
| 领域扩展 | **插件架构** | 核心不变，领域无限扩展 |
| 目标用户 | 开发者 + 领域专家 | 不需要形式化方法背景 |
