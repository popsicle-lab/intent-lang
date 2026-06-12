# Changelog

All notable changes to intent-lang are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

- **Runtime dependency**: `intent check` requires [Z3](https://github.com/Z3Prover/z3) on `PATH`.
- **Scope**: v0.1.0 verifies requirement consistency, not implementation correctness.

[0.1.0]: https://github.com/popsicle-lab/intent-lang/releases/tag/v0.1.0
