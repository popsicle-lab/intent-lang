# Changelog

All notable changes to intent-lang are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-15

### Added

- **State-machine visualization + liveness checks** — `intent-lang-visualizer`
  derives a lifecycle state machine from `require pre-state → ensure post-state`
  clauses (replacing the noisy N×N intent graph in `--all` output); a new
  `--check-states` flag runs structural liveness analysis (unreachable states,
  stuck states, states that cannot reach a terminal) and exits non-zero on
  findings.
- **Visualization annotations** — non-verified authoring hints consumed by
  `intent-lang-visualizer`: `@capability("group")` / `@guardrail("group")` on
  `goal`s drive theme-clustered subgraphs (capability vs. guardrail coloring,
  cross-topic-shared and unclaimed blocks), and `@doc("...")` on `intent`s /
  `goal`s renders a legend table beneath the Goal Graph and State Machine plus
  hover tooltips in the interactive HTML. Grammar, SPEC §6.7.1 and the visualizer
  GUIDE updated; files without the annotations fall back to the flat layout.

- **Vacuity check (V0020)** — a "verified" intent now additionally requires its own
  clause set to be satisfiable; self-contradictory intents (e.g. `require amount > 0`
  together with `require amount < 0`) report `self-contradictory` instead of a vacuous
  green (rfc-modeling-integrity D1).
- **Clause labels & stable IDs** — `ensure debit: ...` optional labels; IDs are
  name-first (`TransferSafe/debit`) with positional fallback (`TransferSafe/ensure[0]`).
  Duplicate labels are `E0006`; unlabeled business-rule clauses get an `H0001` hint (D4).
- **`modifies` clause + frame semantics** — state outside the frame is provably
  unchanged (`x' == x` assumed and asserted in acceptance tests); frame inferred from
  primed ensure paths when omitted; `modifies *` opts out; primed ensure paths must be
  within an explicit `modifies` (`E0007`) (D2).
- **`require ... else reject`** — business-rule requires: violation must observably
  reject the operation and leave all state unchanged. Each marked clause emits an extra
  reject-branch VC and a generated negative test (D3).
- **`example` blocks** — specification by example: `intent check` substitutes the
  author's concrete values into every clause via Z3 (`V0021` pinpoints the contradicted
  clause); examples also become the first generated pytest cases (D5).
- **Executability classification** — every clause is statically classified
  machine / manual (quantifiers → manual until `state` semantics land); typecheck warns
  with `W0011`; testspec rows now carry `clause_ids` and `executability` (D7).
- **Z3 witness solving** — happy / negative / boundary test inputs solved from the
  same SMT encoding used for consistency checking (`witness` module, D8).
- **`intent-lang-accept` crate + `intent accept gen/run`** — the executable acceptance
  pipeline: `.intent.bind.toml` binding format, deterministic pytest codegen, JUnit
  merge back onto clause IDs, goal rollup via `realized_by`, `intent.acceptance_report`
  artifact, strict/lenient CI gate (rfc-executable-acceptance M-A1).
- **Acceptance demo** — `examples/acceptance/`: bank transfer intent + hand-written
  binding + ~50-line Python demo with a seedable bug (`BANK_DEMO_BUGGY=1`); the report
  attributes the failure to the specific `TransferSafe/debit` clause with exit code 1.
- **Agent Skills for the LLM workflow (M-A2)** — `.agents/skills/write-intent`
  (natural-language requirements → `.intent` with the `intent check` counterexample
  loop) and `.agents/skills/implement-testspec` (testspec → binding draft →
  `intent accept gen/run` → clause-level triage). Each skill declares an
  anti-collusion boundary: they must run in separate sessions and may not touch the
  other side's artifacts (rfc-executable-acceptance M-A2, realized as skills instead
  of an `intent bind` built-in).

## [0.1.3] - 2026-06-12

### Changed

- **Crate names** — all published crates now use the `intent-lang-*` prefix:
  - `intent-lang-syntax`, `intent-lang-core`, `intent-lang-visualizer`, `intent-lang-cli`
- **Library crate names** — `intent_lang_syntax`, `intent_lang_core`, `intent_lang_visualizer`
- **Visualizer binary** — renamed to `intent-lang-visualizer` (CLI binary remains `intent`)

### Deprecated

- Crates.io packages `intent-syntax`, `intent-core`, and `intent-visualizer` (v0.1.2) are superseded by the `intent-lang-*` names.

## [0.1.2] - 2026-06-12

### Added

- **`intent-lang-visualizer` library** — publishable crate with `render`, `render_mermaid`, `render_mermaid_raw`, and graph builders for Rust integrations.
- **Crates.io publishing** — workspace crates `intent-lang-syntax`, `intent-lang-core`, `intent-lang-visualizer`, and `intent-lang-cli` with shared metadata.
- **CLI `--format mermaid-raw`** — Mermaid diagram body without Markdown fences.

### Changed

- `intent-lang-visualizer` refactored: logic moved from binary-only `main.rs` into `lib.rs` public API.

## [0.1.1] - 2026-06-12

### Changed

- **Bundled Z3**: `intent check` now uses in-process Z3 via the `z3` crate (`vendored` feature). Release binaries no longer require a separate Z3 installation.
- **Release artifacts**: macOS x86_64 is built via cross-compilation on `macos-latest` (no `macos-13` runner).

### Notes

- **Building from source** requires [CMake](https://cmake.org/) and a C++ toolchain (Z3 is compiled and linked statically at build time).

## [0.1.0] - 2026-06-12

First public release — requirements modeling DSL with Z3-backed consistency checking.

### Added

- **Core language**: `type`, `enum`, `safety`, `intent`, `theorem`, `goal`, `coverage`
- **Lifecycle annotations**: `@tobe` (target state) and `@asis` (legacy behavior)
- **CLI** (`intent` binary):
  - `check` — parse, type-check, and verify with Z3
  - `parse` — dump AST
  - `coverage` — completeness dimension analysis
  - `testspec` — emit test scenario specifications
  - `diff` / `impact` — change classification and impact analysis
  - `explain` — plain-English rendering of intents
  - `--format json` on supported commands
- **Examples**: basics, smarthome, requirements (billing, access-control)
- **Docs**: language spec, positioning, IDD software guide, RFCs
- **Editor support**: TextMate grammar (`tools/grammar/intent.tmLanguage.json`)

### Notes

- **Runtime dependency**: v0.1.0 required [Z3](https://github.com/Z3Prover/z3) on `PATH` (removed in v0.1.1).
- **Scope**: v0.1.0 verifies requirement consistency, not implementation correctness.

[0.1.1]: https://github.com/popsicle-lab/intent-lang/releases/tag/v0.1.1
[0.1.0]: https://github.com/popsicle-lab/intent-lang/releases/tag/v0.1.0
