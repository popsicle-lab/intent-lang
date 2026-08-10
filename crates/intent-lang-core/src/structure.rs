//! Structural checks for `intent check` (RFC: workflow-hardening D1/D2/D10).
//!
//! Z3 verifies that clauses are mutually consistent. It says nothing about
//! whether the file *models the domain's lifecycle at all* — a specification
//! made of disconnected boolean flags, with no goal claiming any of it, passes
//! verification while being useless as requirements. These checks close that
//! gap using plain AST structure (derivation lives in
//! `intent_lang_syntax::structure`).
//!
//! # Severity policy
//!
//! Only two findings are unconditional errors, because only they are
//! unambiguous defects:
//!
//! - `S0007` a single intent asserts two mutually exclusive next-states;
//! - `S0004` a state is unreachable **and** the file does have creation edges.
//!
//! Note that `S0004` weakens as entry points multiply — a state with its own
//! creation edge is trivially reachable — which is why `S0008` reports the
//! multiplicity itself rather than trusting reachability to catch it.
//!
//! Everything else is a warning by default and an error under `--strict`.
//! The reason is that the tool cannot distinguish a defect from a legitimate
//! modeling choice: a file may model only the middle of a lifecycle (creation
//! happens upstream), and a long-lived entity (`Active ↔ Frozen`) has no
//! terminal state by design. Reporting those as errors would push authors to
//! invent a fake bootstrap operation or a fake terminal state — corrupting the
//! requirements to satisfy the tool. Skills that want the strict reading ask
//! for it explicitly with `--strict`.

use std::collections::BTreeSet;

use intent_lang_syntax::ast::{Declaration, Program, Span};
use intent_lang_syntax::structure::{
    analyze_state_machine, build_state_machine_for, intents_without_example, lifecycle_enums,
    unclaimed_realizables, RealizableKind, StateMachine,
};

use crate::{DiagLevel, Diagnostic};

/// Summary of the structural checks, for `--format json` telemetry.
#[derive(Debug, Default, serde::Serialize)]
pub struct StructureSummary {
    /// Enums carrying `@lifecycle`.
    pub lifecycles: Vec<String>,
    pub unclaimed_intents: Vec<String>,
    pub unclaimed_safeties: Vec<String>,
    pub intents_without_example: Vec<String>,
    pub states_unreachable: Vec<String>,
    pub lifecycles_without_creation: Vec<String>,
    pub lifecycles_with_multiple_entries: Vec<String>,
    pub lifecycles_without_terminal: Vec<String>,
    pub states_trapped: Vec<String>,
    pub self_contradictory_intents: Vec<String>,
}

/// Run every structural check. `strict` upgrades the ambiguous findings
/// (see module docs) from warning to error.
pub fn check_structure(program: &Program, strict: bool) -> (Vec<Diagnostic>, StructureSummary) {
    let mut diags = Vec::new();
    let mut summary = StructureSummary::default();
    let soft = if strict {
        DiagLevel::Error
    } else {
        DiagLevel::Warning
    };

    // ── Goal coverage (S0001) ──
    for u in unclaimed_realizables(program) {
        let kind = match u.kind {
            RealizableKind::Intent => {
                summary.unclaimed_intents.push(u.name.clone());
                "intent"
            }
            RealizableKind::Safety => {
                summary.unclaimed_safeties.push(u.name.clone());
                "safety"
            }
        };
        diags.push(Diagnostic {
            level: soft,
            code: "S0001".to_string(),
            message: format!("{kind} `{}` is not claimed by any goal", u.name),
            span: u.span,
            notes: vec![
                "no `goal` lists it in `realized_by` — either a goal forgot its \
                 realizer, or this rule exists without a stated purpose"
                    .to_string(),
            ],
        });
    }

    // ── Example coverage (S0002) ──
    //
    // `@asis` intents are exempt. They record what existing code already does,
    // so the authoritative examples are production data and existing tests,
    // harvested later by the acceptance step. Demanding one per intent at
    // modeling time leaves an agent two options — invent values, or never pass
    // the gate — and inventing them is worse than their absence. Measured on a
    // real reverse-modeled service: 100% of intents were `@asis` and 23 of 29
    // lacked an example, which drowned out the findings that actually
    // distinguish sound modeling from unsound.
    let asis: BTreeSet<&str> = program
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Intent(i)
                if crate::analysis::intent_lifecycle(i) == crate::analysis::Lifecycle::AsIs =>
            {
                Some(i.name.as_str())
            }
            _ => None,
        })
        .collect();

    for (name, span) in intents_without_example(program) {
        if asis.contains(name.as_str()) {
            continue;
        }
        summary.intents_without_example.push(name.clone());
        diags.push(Diagnostic {
            level: soft,
            code: "S0002".to_string(),
            message: format!("intent `{name}` has no `example` block"),
            span,
            notes: vec!["an example with real business values guards against \
                 formalization drift (Z3 substitutes it into every clause) and \
                 seeds acceptance data"
                .to_string()],
        });
    }

    // ── Lifecycle checks (S0003–S0007) ──
    let lifecycles = lifecycle_enums(program);
    summary.lifecycles = lifecycles.clone();
    for enum_name in &lifecycles {
        let sm = build_state_machine_for(program, enum_name);
        let span = enum_span(program, enum_name).unwrap_or(Span::new(0, 0));
        lifecycle_diags(&sm, enum_name, span, soft, &mut diags, &mut summary);
    }

    (diags, summary)
}

fn lifecycle_diags(
    sm: &StateMachine,
    enum_name: &str,
    span: Span,
    soft: DiagLevel,
    diags: &mut Vec<Diagnostic>,
    summary: &mut StructureSummary,
) {
    // S0007: mirrors the verifier's V0020 at the structural level.
    for c in &sm.conflicts {
        summary.self_contradictory_intents.push(c.intent.clone());
        diags.push(Diagnostic {
            level: DiagLevel::Error,
            code: "S0007".to_string(),
            message: format!(
                "intent `{}` unconditionally asserts several `{enum_name}` next-states at once: {}",
                c.intent,
                c.targets.join(", ")
            ),
            span: span.clone(),
            notes: vec!["these ensures can never hold together. If the source \
                 requirement really does state both, leave them and let \
                 stakeholders decide — do not silently pick one"
                .to_string()],
        });
    }

    if sm.states.is_empty() {
        diags.push(Diagnostic {
            level: soft,
            code: "S0003".to_string(),
            message: format!("`@lifecycle enum {enum_name}` drives no transition"),
            span,
            notes: vec![
                "no intent pairs `require x == Variant` with `ensure x' == Variant` \
                 over this enum — the declared lifecycle is inert"
                    .to_string(),
            ],
        });
        return;
    }

    let report = analyze_state_machine(sm);

    if !report.has_creation {
        summary
            .lifecycles_without_creation
            .push(enum_name.to_string());
        diags.push(Diagnostic {
            level: soft,
            code: "S0003".to_string(),
            message: format!("`{enum_name}` has no creation edge"),
            span: span.clone(),
            notes: vec![
                "every intent touching this lifecycle requires a source state, so \
                 nothing can enter it. Add a bootstrap intent that sets the initial \
                 state without requiring one — unless creation genuinely happens \
                 outside this file"
                    .to_string(),
            ],
        });
    }

    // S0008: a lifecycle with several entry points. Found on real data, where
    // it separated every soundly modeled lifecycle (exactly one creation edge)
    // from every broken one (two and three). It is the one machine-visible form
    // of the boolean-flag anti-pattern: when an intent asserts `phase' == X` but
    // sources its precondition from a flag rather than from `phase`, the edge
    // loses its origin and shows up as a second way to bring the entity into
    // existence — including, in the case that surfaced this, directly into the
    // terminal state.
    if report.creation_targets.len() > 1 {
        let entries = report.creation_targets.join(", ");
        summary
            .lifecycles_with_multiple_entries
            .push(enum_name.to_string());
        diags.push(Diagnostic {
            level: soft,
            code: "S0008".to_string(),
            message: format!("`{enum_name}` has {} entry points: {entries}", report.creation_targets.len()),
            span: span.clone(),
            notes: vec![
                format!(
                    "these intents set `{enum_name}` without requiring a source state: \
                     {}. Usually one of them means \"continue from the previous phase\" \
                     but tests a boolean flag instead of the phase itself, which cuts \
                     the chain and makes the unreachable-state check vacuous. If the \
                     lifecycle genuinely has several entry points — an entity created \
                     either by signup or by import — this finding is noise",
                    sm.creation
                        .iter()
                        .map(|c| c.label.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ],
        });
    }

    for state in &report.unreachable_from_initial {
        summary
            .states_unreachable
            .push(format!("{enum_name}::{state}"));
        diags.push(Diagnostic {
            level: DiagLevel::Error,
            code: "S0004".to_string(),
            message: format!("`{enum_name}::{state}` is unreachable from creation"),
            span: span.clone(),
            notes: vec![
                "the lifecycle has creation edges but no path reaches this state — \
                 either an operation is missing, or the variant is dead"
                    .to_string(),
            ],
        });
    }

    if !report.has_terminal {
        summary
            .lifecycles_without_terminal
            .push(enum_name.to_string());
        diags.push(Diagnostic {
            level: soft,
            code: "S0005".to_string(),
            message: format!("`{enum_name}` has no terminal state"),
            span: span.clone(),
            notes: vec![
                "every state has an outgoing transition, so the lifecycle never \
                 ends. Legitimate for long-lived entities (e.g. Active ↔ Frozen)"
                    .to_string(),
            ],
        });
    }

    if !report.cannot_reach_terminal.is_empty() {
        for state in &report.cannot_reach_terminal {
            summary.states_trapped.push(format!("{enum_name}::{state}"));
        }
        diags.push(Diagnostic {
            level: soft,
            code: "S0006".to_string(),
            message: format!(
                "`{enum_name}` states cannot reach any terminal state: {}",
                report.cannot_reach_terminal.join(", ")
            ),
            span,
            notes: vec![
                "the entity can get trapped in a cycle and never complete its \
                 lifecycle"
                    .to_string(),
            ],
        });
    }
}

fn enum_span(program: &Program, name: &str) -> Option<Span> {
    program.declarations.iter().find_map(|d| match &d.node {
        Declaration::Enum(e) if e.name == name => Some(d.span.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        intent_lang_syntax::parse(src).expect("parse")
    }

    fn codes(src: &str, strict: bool) -> Vec<(String, DiagLevel)> {
        let (diags, _) = check_structure(&parse(src), strict);
        diags.into_iter().map(|d| (d.code, d.level)).collect()
    }

    const CLEAN: &str = r#"
        @lifecycle
        enum S { Draft, Done }
        type Doc { status: S }
        goal "docs get published" { realized_by: [Create, Finish] }
        intent Create(d: Doc) { ensure d.status' == Draft }
        intent Finish(d: Doc) {
          require d.status == Draft else reject
          ensure d.status' == Done
        }
        example Create "new draft" { given: { d.status: Done } expect: { d.status': Draft } }
        example Finish "publish" { given: { d.status: Draft } expect: { d.status': Done } }
    "#;

    #[test]
    fn clean_program_has_no_findings_even_under_strict() {
        assert!(codes(CLEAN, true).is_empty());
    }

    #[test]
    fn no_lifecycle_means_no_state_machine_findings() {
        // Transfer-style domain: no lifecycle at all is the correct model,
        // so the tool must stay silent about state machines.
        let src = r#"
            type Account { balance: Int }
            goal "money is conserved" { realized_by: [Transfer] }
            intent Transfer(a: Account) {
              require funds: a.balance >= 10 else reject
              ensure debit: a.balance' == a.balance - 10
            }
            example Transfer "ten" { given: { a.balance: 30 } expect: { a.balance': 20 } }
        "#;
        assert!(codes(src, true).is_empty());
    }

    #[test]
    fn unclaimed_intent_is_warning_by_default_error_under_strict() {
        let src = r#"
            type A { x: Int }
            intent Lonely(a: A) { ensure a.x' == a.x + 1 }
            example Lonely "bump" { given: { a.x: 1 } expect: { a.x': 2 } }
        "#;
        assert_eq!(
            codes(src, false),
            vec![("S0001".into(), DiagLevel::Warning)]
        );
        assert_eq!(codes(src, true), vec![("S0001".into(), DiagLevel::Error)]);
    }

    /// The shape that surfaced S0008: a four-state lifecycle whose middle
    /// transitions test boolean flags instead of the phase, so two of them look
    /// like ways to create the entity — one of them straight into the last state.
    const SEVERED_CHAIN: &str = r#"
        @lifecycle
        enum CleanPhase { Initial, DevicesResolved, LabelsProcessed, CleanCompleted }
        type Ctx { phase: CleanPhase  devicesFound: Bool  externalOk: Bool }
        goal "clean completes" { realized_by: [Bootstrap, Parse, Labels, Complete] }
        @asis
        intent Bootstrap(c: Ctx) { ensure i: c.phase' == Initial }
        @asis
        intent Parse(c: Ctx) {
          require from: c.phase == Initial else reject
          ensure p: c.phase' == DevicesResolved
        }
        @asis
        intent Labels(c: Ctx) {
          require found: c.devicesFound else reject
          ensure l: c.phase' == LabelsProcessed
        }
        @asis
        intent Complete(c: Ctx) {
          require ok: c.externalOk else reject
          ensure d: c.phase' == CleanCompleted
        }
    "#;

    #[test]
    fn severed_lifecycle_chain_reports_multiple_entry_points() {
        let (diags, summary) = check_structure(&parse(SEVERED_CHAIN), false);
        let s0008: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == "S0008").collect();
        assert_eq!(s0008.len(), 1, "got: {diags:?}");
        assert!(
            s0008[0].message.contains("3 entry points"),
            "the count is the signal: {}",
            s0008[0].message
        );
        assert_eq!(
            summary.lifecycles_with_multiple_entries,
            vec!["CleanPhase".to_string()]
        );
        // The point of the check: reachability sees nothing wrong here, because
        // each severed state carries its own creation edge.
        assert!(summary.states_unreachable.is_empty());
    }

    #[test]
    fn multiple_entry_points_are_a_warning_by_default() {
        let (diags, _) = check_structure(&parse(SEVERED_CHAIN), false);
        let d = diags.iter().find(|d| d.code == "S0008").unwrap();
        assert_eq!(d.level, DiagLevel::Warning);

        let (strict, _) = check_structure(&parse(SEVERED_CHAIN), true);
        let d = strict.iter().find(|d| d.code == "S0008").unwrap();
        assert_eq!(d.level, DiagLevel::Error);
    }

    #[test]
    fn a_single_entry_point_is_not_reported() {
        let src = r#"
            @lifecycle
            enum P { Start, Middle, End }
            type Ctx { phase: P }
            goal "flows" { realized_by: [Boot, Advance, Finish] }
            @asis
            intent Boot(c: Ctx) { ensure i: c.phase' == Start }
            @asis
            intent Advance(c: Ctx) {
              require f: c.phase == Start else reject
              ensure m: c.phase' == Middle
            }
            @asis
            intent Finish(c: Ctx) {
              require f: c.phase == Middle else reject
              ensure e: c.phase' == End
            }
        "#;
        let (diags, summary) = check_structure(&parse(src), true);
        assert!(diags.is_empty(), "got: {diags:?}");
        assert!(summary.lifecycles_with_multiple_entries.is_empty());
    }

    #[test]
    fn asis_intents_are_exempt_from_the_example_requirement() {
        let src = r#"
            type A { x: Int }
            goal "g" { realized_by: [Legacy, Promised] }
            @asis
            intent Legacy(a: A) { ensure a.x' == a.x + 1 }
            @tobe
            intent Promised(a: A) { ensure a.x' == a.x + 2 }
        "#;
        let (diags, summary) = check_structure(&parse(src), true);
        assert_eq!(summary.intents_without_example, vec!["Promised".to_string()]);
        assert!(
            diags.iter().all(|d| !d.message.contains("Legacy")),
            "an @asis intent records existing behaviour; its examples come from \
             production data, not from the modeling session"
        );
    }

    #[test]
    fn missing_example_is_warning_by_default() {
        let src = r#"
            type A { x: Int }
            goal "g" { realized_by: [Op] }
            intent Op(a: A) { ensure a.x' == a.x + 1 }
        "#;
        assert_eq!(
            codes(src, false),
            vec![("S0002".into(), DiagLevel::Warning)]
        );
    }

    #[test]
    fn missing_creation_edge_is_warning_not_error() {
        // Modeling only the middle of a lifecycle is legitimate; forcing an
        // error here would push authors to invent a fake bootstrap intent.
        let src = r#"
            @lifecycle
            enum S { Open, Done }
            type Doc { status: S }
            goal "g" { realized_by: [Finish] }
            intent Finish(d: Doc) {
              require d.status == Open else reject
              ensure d.status' == Done
            }
            example Finish "f" { given: { d.status: Open } expect: { d.status': Done } }
        "#;
        let default = codes(src, false);
        assert!(default.contains(&("S0003".into(), DiagLevel::Warning)));
        assert!(default.iter().all(|(_, l)| *l == DiagLevel::Warning));
        assert!(codes(src, true).iter().all(|(_, l)| *l == DiagLevel::Error));
    }

    #[test]
    fn unreachable_state_is_error_even_without_strict() {
        let src = r#"
            @lifecycle
            enum S { Draft, Orphan, Done }
            type Doc { status: S }
            goal "g" { realized_by: [Create, Finish, Leave] }
            intent Create(d: Doc) { ensure d.status' == Draft }
            intent Finish(d: Doc) {
              require d.status == Draft else reject
              ensure d.status' == Done
            }
            intent Leave(d: Doc) {
              require d.status == Orphan else reject
              ensure d.status' == Done
            }
            example Create "c" { given: { d.status: Done } expect: { d.status': Draft } }
            example Finish "f" { given: { d.status: Draft } expect: { d.status': Done } }
            example Leave "l" { given: { d.status: Orphan } expect: { d.status': Done } }
        "#;
        assert!(codes(src, false).contains(&("S0004".into(), DiagLevel::Error)));
    }

    #[test]
    fn self_contradictory_transition_is_error_even_without_strict() {
        let src = r#"
            @lifecycle
            enum S { Open, Closed, Exception }
            type Doc { status: S }
            goal "g" { realized_by: [Close] }
            intent Close(d: Doc) {
              require d.status == Open else reject
              ensure a: d.status' == Closed
              ensure b: d.status' == Exception
            }
            example Close "c" { given: { d.status: Open } expect: { d.status': Closed } }
        "#;
        assert!(codes(src, false).contains(&("S0007".into(), DiagLevel::Error)));
    }

    #[test]
    fn summary_records_lifecycles_for_telemetry() {
        let (_, summary) = check_structure(&parse(CLEAN), false);
        assert_eq!(summary.lifecycles, vec!["S".to_string()]);
        assert!(summary.unclaimed_intents.is_empty());
    }
}
