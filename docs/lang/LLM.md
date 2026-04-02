# intent-lang 与大模型 (LLM)

> intent-lang 可能是目前最适合 LLM 生成的形式化语言。

---

## 为什么 LLM 友好？

| 特性 | 原因 |
|------|------|
| **关键字少** | 7 个核心词，LLM 只需学一个模板 |
| **接近自然语言** | `require amount > 0` 几乎就是英语 |
| **不需要写实现** | LLM 最容易在算法实现上犯错，intent-lang 不需要 |
| **结构固定** | 每个 intent：名字 → 参数 → require → ensure → invariant |
| **短** | 一个 intent 通常 5-10 行 |

### 与其他形式化语言的生成难度对比

| 语言 | 生成难度 | 验证能力 | 自我修正 |
|------|---------|---------|---------|
| Python | ⭐⭐ | ❌ 无 | ❌ 难 |
| Rust | ⭐⭐⭐ | ⭐⭐ 类型 | ⚠️ 一般 |
| Dafny | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ SMT | ⭐⭐⭐ |
| TLA+ | ⭐⭐⭐⭐ | ⭐⭐⭐ 模型检查 | ⭐⭐ |
| Lean 4 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐ |
| **intent-lang** | **⭐⭐** | **⭐⭐⭐⭐** | **⭐⭐⭐⭐** |

**独一无二的组合：低生成难度 + 高验证能力 + 高自我修正能力。**

---

## 核心机制：LLM 犯错，Z3 兜底

```
LLM 生成的代码 → SMT 验证 → 通过？
                              │
                ┌──────────────┼──────────────┐
                ▼              ▼              ▼
              ✅ 安全         ❌ 反例        ⚠️ timeout
              直接用          反馈给 LLM      人工审查
                             重新生成
```

### 示例：闭环修正

```intent
// LLM 第 1 轮（有 bug：忘了检查余额）
intent TransferSafe(sender: Account, amount: Int) {
  require amount > 0
  ensure sender.balance' == sender.balance - amount
  invariant sender.balance' >= 0
}
```

```bash
$ intent check
  ❌ Counterexample: sender.balance = 3, amount = 10
     sender.balance' = -7, violates invariant
```

```intent
// LLM 第 2 轮（看到反例，自动修正）
+ require sender.balance >= amount
```

```bash
$ intent check → ✅ verified
```

**为什么其他语言做不到这个闭环？**

| 语言 | LLM 犯错后 |
|------|-----------|
| Python | 编译通过，运行时才出错，可能根本不被发现 |
| Lean 4 | 错误信息晦涩（proof state），LLM 很难自我修正 |
| TLA+ | 输出长 trace，LLM 难以解析 |
| **intent-lang** | **反例是 `variable=value`，LLM 一看就懂** |

---

## LLM-Friendly 设计原则

| 原则 | 说明 |
|------|------|
| 语法接近自然语言 | `require balance >= amount`，不是 `/\ balance >= amount` |
| 结构固定可预测 | LLM 只需学一个模板 |
| 不需要写 How | 避开 LLM 最容易犯错的算法实现 |
| 错误反馈可机器解读 | 反例是结构化的 `variable = value` |
| 验证结果二元 | ✅ 或 ❌，没有"大部分对"的灰色地带 |
| 安全网兜底 | 最坏是"验证失败"，不是"静默出错" |

---

## 待改进的点

| 问题 | 描述 | 可能改进 |
|------|------|---------|
| `x'` 语义 | LLM 可能不理解 primed 变量 | 考虑 `after(x)` 或 `next(x)` |
| `::` 分隔符 | `forall x: T :: P(x)` 不常见 | 考虑 `forall x: T, P(x)` |
| 隐式 safety | LLM 看不到但会影响验证 | 报告中显示完整约束 |
| 训练数据 | 新语言，LLM 没见过 | 需要丰富的示例库用于 few-shot |
