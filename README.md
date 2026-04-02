# intent-lang

A universal intent-based specification language with formal verification. Declare **what** you want, not **how** — the system automatically proves it correct.

## What is intent-lang?

intent-lang lets you declare logical intents (preconditions, postconditions, invariants) and automatically verifies them using an SMT solver (Z3). Combined with an LLM translation layer, you can go from natural language to formally verified specifications.

```intent
type Account {
  balance: Int
  owner: String
}

intent TransferSafe(sender: Account, receiver: Account, amount: Int) {
  require amount > 0
  require sender.balance >= amount
  ensure sender.balance' == sender.balance - amount
  ensure receiver.balance' == receiver.balance + amount
  invariant sender.balance' >= 0
}

theorem TransferPreservesTotal {
  forall s: Account, r: Account, a: Int ::
    TransferSafe(s, r, a) ==>
      s.balance' + r.balance' == s.balance + r.balance
}
```

```bash
$ intent check transfer.intent
  ✅ intent TransferSafe       — verified
  ✅ theorem TransferPreservesTotal — proved
```

## Key Features

- **Declare, don't implement** — write conditions (`require`/`ensure`/`invariant`), not code
- **Automatic verification** — SMT solver (Z3) proves correctness or generates counterexamples
- **LLM-assisted** — generate intent code from natural language descriptions
- **Domain plugins** — extensible type system with domain-specific types, safety rules, and axioms
- **Multi-level intents** — from user stories to component specs, with refinement proofs between layers

## Documentation

| Document | Description |
|----------|-------------|
| [PLAN.md](PLAN.md) | Implementation plan and milestones |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Design decisions: Intent approaches & verification levels |
| [docs/DESIGN.md](docs/DESIGN.md) | Language design and architecture |
| [docs/PLUGINS.md](docs/PLUGINS.md) | Domain plugin system |
| [docs/EXECUTION.md](docs/EXECUTION.md) | 4-layer execution architecture (Intent → Plan → Execute → Verify) |
| [docs/VALUE_SMARTHOME.md](docs/VALUE_SMARTHOME.md) | Value analysis: smart home scenario |
| [docs/PLATFORMS.md](docs/PLATFORMS.md) | Alexa / Mi Home / Alice / HomeKit architecture comparison |
| [docs/LLM_FRIENDLY.md](docs/LLM_FRIENDLY.md) | Why intent-lang is the most LLM-friendly formal language |
| [examples/USAGE.md](examples/USAGE.md) | CLI usage walkthrough |
| [examples/comparison/](examples/comparison/) | Comparison with Lean 4 and TLA+ |

## Quick Links

- [Transfer example](examples/transfer.intent) — bank transfer with bug detection
- [Auth example](examples/auth.intent) — login lockout and access control
- [Sorting example](examples/sorting.intent) — sort specification with idempotence theorem
- [Smart home example](examples/smarthome.intent) — voice control with safety rules

## Tech Stack

- **Language**: Rust
- **Parser**: `logos` (lexer) + hand-written recursive descent
- **Verification**: Z3 SMT solver via SMT-LIB2
- **LLM**: OpenAI/Anthropic API for natural language → intent translation
- **Future**: WASM compilation for web playground

## License

TBD
