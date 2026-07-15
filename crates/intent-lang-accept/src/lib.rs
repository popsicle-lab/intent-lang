//! intent-lang-accept — the executable acceptance pipeline
//! (docs/rfc-executable-acceptance.md, goal B).
//!
//! Boundary with `intent-lang-core` (D11): core does pure analysis —
//! testspec, clause IDs, Z3 witnesses. This crate consumes them and owns
//! everything with side effects: binding files, codegen, process
//! execution, report artifacts. Zero LLM at execution time (D4/D6).

pub mod binding;
pub mod codegen;
pub mod report;
