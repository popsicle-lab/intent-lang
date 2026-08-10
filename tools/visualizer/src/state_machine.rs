//! State-machine view — derivation lives in `intent_lang_syntax::structure`.
//!
//! The derivation (which `require`/`ensure` pairs form which transitions) and
//! the liveness analysis used to live here, which meant the checks only ran
//! when someone remembered to draw a diagram. They now sit in the syntax crate
//! so `intent check` gates on them (see `intent_lang_core::structure`), and
//! this module is left with what a visualizer should own: turning the derived
//! machine into a picture.
//!
//! Re-exported rather than wrapped so both consumers read the same
//! implementation — a second copy would drift, and the two would disagree
//! about whether a file is sound.

pub use intent_lang_syntax::structure::{
    analyze_state_machine, build_state_machine, build_state_machine_for, lifecycle_enums,
    lifecycle_state_machines, terminal_states, StateConflict, StateMachine, StateMachineReport,
    StateTransition,
};

impl crate::GraphData for StateMachine {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::MermaidRenderable;
    use intent_lang_syntax::ast::Program;

    const SRC: &str = r#"
        @lifecycle
        enum S { Draft, Open, Done }
        type Doc { status: S }
        intent Create(d: Doc) {
          ensure d.status' == Draft
        }
        intent Publish(d: Doc) {
          require d.status == Draft else reject
          ensure d.status' == Open
        }
        intent Finish(d: Doc) {
          require d.status == Open else reject
          ensure d.status' == Done
        }
    "#;

    fn parse(src: &str) -> Program {
        intent_lang_syntax::parse(src).expect("parse")
    }

    #[test]
    fn renders_state_diagram() {
        let sm = build_state_machine(&parse(SRC));
        let m = sm.to_mermaid();
        assert!(m.contains("stateDiagram-v2"));
        assert!(m.contains("[*] --> Draft"));
        assert!(m.contains("Done --> [*]"));
    }

    #[test]
    fn renders_transition_labels() {
        let m = build_state_machine(&parse(SRC)).to_mermaid();
        assert!(m.contains("Draft --> Open"));
        assert!(m.contains("Publish"));
    }

    #[test]
    fn no_state_enum_degrades_gracefully() {
        let src = "type A { x: Int }\nintent Bump(a: A) { ensure a.x' == a.x + 1 }";
        let sm = build_state_machine(&parse(src));
        assert!(sm.state_enum.is_none());
        assert!(sm.to_mermaid().contains("stateDiagram-v2"));
    }

    #[test]
    fn undeclared_enum_still_drawn_via_legacy_heuristic() {
        // Diagrams stay useful for files written before `@lifecycle` existed;
        // it is the *gating* that requires an explicit declaration.
        let src = r#"
            enum S { Draft, Done }
            type Doc { status: S }
            intent Create(d: Doc) { ensure d.status' == Draft }
            intent Finish(d: Doc) {
              require d.status == Draft else reject
              ensure d.status' == Done
            }
        "#;
        let sm = build_state_machine(&parse(src));
        assert_eq!(sm.state_enum.as_deref(), Some("S"));
    }

    #[test]
    fn conflict_is_available_for_flagging() {
        let src = r#"
            @lifecycle
            enum S { A, B, C }
            type X { status: S }
            intent Start(x: X) { ensure x.status' == A }
            intent Bad(x: X) {
              require x.status == A else reject
              ensure b: x.status' == B
              ensure c: x.status' == C
            }
        "#;
        let sm = build_state_machine(&parse(src));
        assert_eq!(sm.conflicts.len(), 1);
        assert!(crate::mermaid::state_conflict_note(&sm).is_some());
    }
}
