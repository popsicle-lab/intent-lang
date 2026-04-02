# intent-lang

A universal intent-based specification language with formal verification.
Declare **what** you want — the system automatically proves it correct.

```intent
intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  require amount > 0
  require sender.balance >= amount
  ensure sender.balance' == sender.balance - amount
  ensure receiver.balance' == receiver.balance + amount
  invariant sender.balance' >= 0
}
```

```bash
$ intent check transfer.intent
  ✅ intent TransferSafe — verified
```

No implementation. No proof. No tests. Z3 verifies it automatically.

---

## Key Features

- **Declare, don't implement** — write `require`/`ensure`/`invariant`, not algorithms
- **Automatic verification** — Z3 SMT solver proves correctness or finds counterexamples
- **LLM-assisted** — natural language → intent code → auto-verified
- **Domain plugins** — core language stays fixed, domains extend via plugins
- **Multi-level refinement** — business → API → component intents, each level verified

---

## Documentation

### 📘 Language

| Document | What you'll learn |
|----------|-------------------|
| [5 分钟概览](docs/lang/README.md) | intent-lang 是什么，核心概念速查 |
| [语法规范](docs/lang/SPEC.md) | 完整语法 EBNF、表达式优先级、SMT 编码 |
| [设计决策](docs/lang/DECISIONS.md) | 为什么选混合方式、SMT 验证、Rust |
| [与大模型的关系](docs/lang/LLM.md) | 为什么是最 LLM-friendly 的形式化语言 |

### 🏗️ Architecture

| Document | What you'll learn |
|----------|-------------------|
| [插件系统](docs/architecture/PLUGINS.md) | 4 层插件结构、5 个领域示例 |
| [执行架构](docs/architecture/EXECUTION.md) | 意图→规划→执行→验证的 4 层桥接 |

### 🎯 Use Cases

| Document | What you'll learn |
|----------|-------------------|
| [软件开发](docs/software/README.md) | PRD→意图→验证→生成测试/断言/API 契约 |
| [智能家居](docs/smarthome/README.md) | 安全验证、冲突检测、可解释性、平台对比 |

### 📂 Examples

| File | Description |
|------|-------------|
| [transfer.intent](examples/basics/transfer.intent) | Bank transfer with bug detection |
| [auth.intent](examples/basics/auth.intent) | Login lockout & access control |
| [sorting.intent](examples/basics/sorting.intent) | Sort specification |
| [smarthome.intent](examples/smarthome/smarthome.intent) | Voice control with safety rules |
| [comparison/](examples/comparison/) | Side-by-side with Lean 4 & TLA+ |
| [CLI usage](examples/USAGE.md) | Full command-line walkthrough |

---

## Implementation

| | |
|---|---|
| **Language** | Rust |
| **Parser** | `logos` + recursive descent |
| **Verification** | Z3 via SMT-LIB2 |
| **LLM** | OpenAI / Anthropic API |
| **Roadmap** | [PLAN.md](PLAN.md) |

## License

TBD
