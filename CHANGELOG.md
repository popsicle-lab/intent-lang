# Changelog

All notable changes to intent-lang are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
