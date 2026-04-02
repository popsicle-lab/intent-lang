# intent-lang 与大模型 (LLM) 的关系

## 核心结论

intent-lang 可能是**目前最适合 LLM 生成的形式化语言**。

原因：语法简单让 LLM 容易生成，SMT 验证让错误无处藏身，结构化反例让 LLM 容易自我修正。

---

## 维度一：LLM 生成 intent-lang 代码容易吗？

**非常容易。** 原因：

| 特性 | 为什么 LLM 友好 |
|------|-----------------|
| 关键字少 | 7 个核心词：type/intent/require/ensure/invariant/theorem/forall |
| 结构固定 | 每个 intent 都是：名字 → 参数 → require → ensure → invariant |
| 接近自然语言 | `require amount > 0` 几乎就是英语 |
| 无实现代码 | 不需要写算法/循环/递归（LLM 最容易出错的部分） |
| 短 | 一个 intent 通常 5-10 行，在 LLM 的最佳范围内 |

### 对比其他形式化语言的 LLM 生成难度

```
Lean 4（难度: ⭐⭐⭐⭐⭐）
  theorem transfer_correct ... := by
    unfold transferPre at h       ← LLM 经常搞错 tactic 顺序
    unfold transfer transferPost  ← 需要了解 proof state
    simp; omega                   ← 不确定能不能解决
  问题: LLM 对 proof tactic 的掌握很差

TLA+（难度: ⭐⭐⭐⭐）
  balances' = [balances EXCEPT
      ![sender] = balances[sender] - amount]
  问题: EXCEPT 语法怪异，训练数据中 TLA+ 很少

intent-lang（难度: ⭐⭐）
  intent TransferSafe(sender: Account, amount: Int) {
    require amount > 0
    require sender.balance >= amount
    ensure sender.balance' == sender.balance - amount
  }
  优势: 接近自然语言，结构简单固定
```

---

## 维度二：LLM 生成的代码能被信任吗？

**不能信任，但不需要信任。** 这是 intent-lang 的杀手特性：

```
LLM 生成的代码 → SMT 自动验证 → 通过？
                                  │
                    ┌─────────────┼──────────────┐
                    ▼             ▼              ▼
                  ✅ 安全        ❌ 有反例       ⚠️ timeout
                  可以用         反馈给 LLM      人工审查
                                重新生成
```

### 示例：LLM 犯错 → SMT 兜底

```intent
// LLM 第 1 轮生成（有 bug：忘了检查余额）
intent TransferSafe(sender: Account, amount: Int) {
  require amount > 0
  ensure sender.balance' == sender.balance - amount
  invariant sender.balance' >= 0
}
```

```bash
$ intent check
  ❌ TransferSafe — verification failed
     Counterexample: sender.balance = 3, amount = 10
     sender.balance' = -7, violates invariant >= 0
```

```intent
// LLM 第 2 轮生成（看到反例后修正）
intent TransferSafe(sender: Account, amount: Int) {
  require amount > 0
  require sender.balance >= amount    // ← 修正
  ensure sender.balance' == sender.balance - amount
  invariant sender.balance' >= 0
}
```

```bash
$ intent check
  ✅ TransferSafe — verified
```

### 为什么其他语言做不到这个闭环？

| 语言 | LLM 犯错后的体验 |
|------|-----------------|
| **Python** | 编译通过，运行时才出错，可能根本不会被发现 |
| **Lean 4** | 编译失败，但错误信息晦涩，LLM 很难自我修正 |
| **TLA+** | model checker 输出长 trace，LLM 难以解析 |
| **intent-lang** | **SMT 立刻指出哪里错 + 给反例 (variable=value)，LLM 一看就懂** |

---

## LLM + intent-lang 闭环工作流

```
用户: "确保转账安全，不能透支"
       │
       ▼
 LLM (Round 1): 生成初始 intent
       │
       ▼
 SMT 验证 → ❌ 反例: balance=3, amount=10
       │
       ▼
 LLM (Round 2): 看到反例，添加 require balance >= amount
       │
       ▼
 SMT 验证 → ✅ verified
       │
       ▼
 用户确认 → 保存
```

---

## LLM-Friendly 语言设计原则

intent-lang 的语法设计遵循以下原则，使其对 LLM 生成天然友好：

| 原则 | 说明 |
|------|------|
| **语法接近自然语言** | `require balance >= amount` 而不是 `/\ balance >= amount` |
| **结构固定可预测** | intent 总是: name → params → require → ensure，LLM 只学一个模板 |
| **不需要写 "How"** | LLM 最容易在算法实现上犯错，intent-lang 不需要写实现 |
| **错误反馈可机器解读** | 反例是 `variable = value`，LLM 可以解析并修正 |
| **验证结果是二元的** | ✅ verified 或 ❌ counterexample，没有灰色地带 |
| **安全网兜底** | 无论 LLM 生成什么，SMT 都会检查。最坏是"验证失败"，不是"静默出错" |

---

## 当前语法中对 LLM 不够友好的点

| 问题 | 描述 | 可能的改进 |
|------|------|----------|
| primed 变量 `x'` | LLM 可能不理解 `'` 语义，容易遗漏 | 考虑替代：`after(x)` 或 `next(x)` |
| 量词分隔符 `::` | `forall x: T :: P(x)` 的 `::` 不常见 | 考虑：`forall x: T, P(x)` 或 `forall x: T where P(x)` |
| 隐式 safety 合并 | LLM 看不到 safety 约束但它们影响验证 | 验证报告应显示完整约束集合 |
| 训练数据缺失 | intent-lang 是新语言，LLM 没见过 | 需要建立丰富的示例库用于 few-shot prompt |

---

## 与现有 LLM 工具链的对比

```
                    生成难度    验证能力    自我修正
                    ─────────  ─────────  ─────────
Python              ⭐⭐       ❌ 无       ❌ 难
TypeScript          ⭐⭐       ⭐ 类型     ⚠️ 一般
Rust                ⭐⭐⭐     ⭐⭐ 类型+借用 ⚠️ 一般
Dafny               ⭐⭐⭐⭐   ⭐⭐⭐⭐ SMT ⭐⭐⭐
TLA+                ⭐⭐⭐⭐   ⭐⭐⭐ 模型检查 ⭐⭐
Lean 4              ⭐⭐⭐⭐⭐  ⭐⭐⭐⭐⭐   ⭐
intent-lang         ⭐⭐       ⭐⭐⭐⭐ SMT ⭐⭐⭐⭐

intent-lang = 低生成难度 + 高验证能力 + 高自我修正能力
              ← 这个组合是独一无二的
```
