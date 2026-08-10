//! Golden pair for the structural gate (RFC: workflow-hardening §5.2).
//!
//! Both fixtures in `fixtures/` come from one real reverse-modeling session on
//! an existing service, recovered from that session's edit history and
//! transposed onto a neutral domain. `provisioning-unsound.intent` is the
//! version the author shipped and then threw away; `provisioning-sound.intent`
//! is the rewrite. Neither was written by anyone who knew what these checks
//! look for — that is the entire point, and it is why this pair carries
//! evidence a hand-written fixture cannot.
//!
//! The claim under test: **`intent check` alone cannot tell the two apart, and
//! the structural gate can.** Both verify green under Z3. If a future change
//! makes the gate agree about both, the gate has stopped discriminating and
//! these tests must fail.

use intent_lang_core::smt::{verify_vc, VerifyResult};
use intent_lang_core::structure::check_structure;
use intent_lang_core::typeck::check_program;
use intent_lang_core::vcgen::generate_vcs;
use intent_lang_core::DiagLevel;
use intent_lang_syntax::ast::Program;

const UNSOUND: &str = include_str!("fixtures/provisioning-unsound.intent");
const SOUND: &str = include_str!("fixtures/provisioning-sound.intent");

fn program(src: &str) -> Program {
    intent_lang_syntax::parse(src).expect("fixture must parse")
}

fn findings(src: &str, strict: bool) -> Vec<(String, DiagLevel)> {
    let (diags, _) = check_structure(&program(src), strict);
    diags.into_iter().map(|d| (d.code, d.level)).collect()
}

/// Every verification condition the file produces, excluding the ones the
/// encoder cannot express (those are skipped by `intent check` too).
fn verification_failures(src: &str) -> Vec<String> {
    let prog = program(src);
    generate_vcs(&prog)
        .iter()
        .filter(|vc| vc.unsupported.is_none())
        .filter_map(|vc| match verify_vc(vc, &prog) {
            VerifyResult::Verified => None,
            other => Some(format!("{}: {other:?}", vc.name)),
        })
        .collect()
}

#[test]
fn both_fixtures_type_check() {
    for (name, src) in [("unsound", UNSOUND), ("sound", SOUND)] {
        let errors: Vec<String> = check_program(&program(src))
            .into_iter()
            .filter(|d| d.level == DiagLevel::Error)
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect();
        assert!(errors.is_empty(), "{name} fixture: {errors:?}");
    }
}

#[test]
fn both_fixtures_verify_green_under_z3() {
    // The premise of the whole RFC: passing verification says nothing about
    // whether the file models the domain. If the unsound fixture ever stops
    // verifying, it has been "fixed" into a different specimen and no longer
    // demonstrates the failure it was collected for.
    for (name, src) in [("unsound", UNSOUND), ("sound", SOUND)] {
        let failures = verification_failures(src);
        assert!(failures.is_empty(), "{name} fixture should verify: {failures:?}");
    }
}

#[test]
fn the_gate_rejects_the_unsound_version() {
    let strict = findings(UNSOUND, true);
    let codes: Vec<&str> = strict.iter().map(|(c, _)| c.as_str()).collect();
    assert!(
        codes.contains(&"S0001"),
        "nine realizers are claimed by no goal; that is the defect the author \
         reported and the gate must name it: {strict:?}"
    );
    assert!(strict.iter().all(|(_, l)| *l == DiagLevel::Error));
}

#[test]
fn the_gate_accepts_the_sound_version() {
    let strict = findings(SOUND, true);
    assert!(strict.is_empty(), "expected a clean gate, got: {strict:?}");
}

#[test]
fn the_pair_is_actually_distinguishable() {
    // Guards against a gate weakened into a no-op, under which the two
    // assertions above would both hold vacuously.
    assert!(!findings(UNSOUND, true).is_empty());
    assert!(findings(SOUND, true).is_empty());
}

#[test]
fn the_sound_version_declares_the_lifecycles_the_unsound_one_lacks() {
    // The substantive modeling difference. The unsound version shredded the
    // registration lifecycle into fourteen booleans on one type, so there is
    // no enum to annotate and no state machine to analyze; the rewrite has two
    // real lifecycles.
    use intent_lang_syntax::structure::{lifecycle_enums, lifecycle_state_machines};

    assert!(lifecycle_enums(&program(UNSOUND)).is_empty());

    let machines = lifecycle_state_machines(&program(SOUND));
    assert_eq!(machines.len(), 2, "one state machine per declared lifecycle");
    assert!(machines.iter().all(|m| !m.transitions.is_empty()));
}

#[test]
fn boolean_flag_modeling_is_the_gate_s_known_blind_spot() {
    // Documented limitation, not an aspiration (RFC §10.1). The gate catches
    // the unsound fixture through its *missing goals*, not through its
    // boolean-flag modeling. Shred a lifecycle into booleans but claim every
    // operation with a goal, and the gate has nothing to say — there is no
    // declared lifecycle, so every state-machine check is silent by design
    // (reporting them would fire on domains that genuinely have no lifecycle).
    //
    // If this test ever fails, some check learned to see the anti-pattern and
    // the skill prose that currently carries this burden can be relaxed.
    let src = r#"
        type Flow { stepAdone: Bool  stepBdone: Bool  stepCdone: Bool }
        goal "flow completes" { realized_by: [DoA, DoB, DoC] }
        @asis
        intent DoA(f: Flow) { ensure a: f.stepAdone' == true }
        @asis
        intent DoB(f: Flow) {
          require prev: f.stepAdone else reject
          ensure b: f.stepBdone' == true
        }
        @asis
        intent DoC(f: Flow) {
          require prev: f.stepBdone else reject
          ensure c: f.stepCdone' == true
        }
    "#;
    let strict = findings(src, true);
    assert!(
        strict.is_empty(),
        "known blind spot has been closed — update RFC §10.1 and the \
         write-intent anti-pattern: {strict:?}"
    );
}
