use intent_lang_core::example::{check_examples, ExampleStatus};
use intent_lang_core::smt::{verify_vc, VerifyResult};
use intent_lang_core::typeck::check_program;
use intent_lang_core::vcgen::{generate_vcs, VcKind};
use intent_lang_core::DiagLevel;
use intent_lang_syntax::parse;

fn verify_file(source: &str) -> Vec<(String, VcKind, VerifyResult)> {
    let prog = parse(source).expect("parse failed");
    let vcs = generate_vcs(&prog);
    vcs.iter()
        .filter(|vc| vc.unsupported.is_none())
        .map(|vc| {
            let result = verify_vc(vc, &prog);
            (vc.name.clone(), vc.kind, result)
        })
        .collect()
}

#[test]
fn transfer_safe_verified() {
    let source = std::fs::read_to_string("../../examples/basics/transfer.intent").unwrap();
    let results = verify_file(&source);

    let safe = results
        .iter()
        .find(|(n, _, _)| n == "TransferSafe")
        .unwrap();
    assert!(
        matches!(safe.2, VerifyResult::Verified),
        "TransferSafe should verify"
    );
}

#[test]
fn transfer_buggy_fails() {
    let source = std::fs::read_to_string("../../examples/basics/transfer.intent").unwrap();
    let results = verify_file(&source);

    let buggy = results
        .iter()
        .find(|(n, _, _)| n == "TransferBuggy")
        .unwrap();
    assert!(
        matches!(buggy.2, VerifyResult::Failed { .. }),
        "TransferBuggy should fail"
    );
}

#[test]
fn auth_intents_verified() {
    let source = std::fs::read_to_string("../../examples/basics/auth.intent").unwrap();
    let results = verify_file(&source);

    for (name, kind, result) in &results {
        if *kind == VcKind::Intent {
            assert!(
                matches!(result, VerifyResult::Verified),
                "intent {name} should verify (no invariants)"
            );
        }
    }
}

#[test]
fn inline_withdraw_without_guard_fails() {
    let source = r#"
type Account {
  balance: Int
}

intent Withdraw(acc: Account, amount: Int) {
  require amount > 0
  ensure acc.balance' == acc.balance - amount
  invariant acc.balance' >= 0
}
"#;
    let results = verify_file(source);
    let w = results.iter().find(|(n, _, _)| n == "Withdraw").unwrap();
    // Fails: acc.balance = 0, amount = 1 → acc.balance' = -1
    assert!(
        matches!(w.2, VerifyResult::Failed { .. }),
        "Withdraw without balance guard should fail"
    );
}

#[test]
fn contradictory_intent_rejected_not_vacuously_verified() {
    // rfc-modeling-integrity D1: contradictory assumes must NOT report Verified.
    let source = r#"
type Account {
  balance: Int
}

intent Contradictory(a: Account, amount: Int) {
  require amount > 0
  require amount < 0
  ensure a.balance' == a.balance - amount
  invariant a.balance' >= 0
}

intent SelfDefeating(a: Account) {
  ensure a.balance' == a.balance + 1
  ensure a.balance' == a.balance - 1
  invariant a.balance' >= 0
}
"#;
    let results = verify_file(source);
    for name in ["Contradictory", "SelfDefeating"] {
        let r = results.iter().find(|(n, _, _)| n == name).unwrap();
        assert!(
            matches!(r.2, VerifyResult::SelfContradictory),
            "{name} should be reported self-contradictory, got {:?}",
            r.2
        );
    }
}

#[test]
fn frame_semantics_prove_unmentioned_state_unchanged() {
    // D2: `a.active` is never primed → it's outside the inferred frame →
    // `a.active' == a.active` is assumed, so the invariant holds.
    let source = r#"
type Account {
  balance: Int
  active: Bool
}

intent Deposit(a: Account, amount: Int) {
  require amount > 0
  ensure a.balance' == a.balance + amount
  invariant a.active' == a.active
}
"#;
    let results = verify_file(source);
    let r = results.iter().find(|(n, _, _)| n == "Deposit").unwrap();
    assert!(
        matches!(r.2, VerifyResult::Verified),
        "frame semantics should prove unmentioned fields unchanged, got {:?}",
        r.2
    );
}

#[test]
fn modifies_wildcard_opts_out_of_frame() {
    // D2: `modifies *` disables the frame → a.active' is unconstrained
    // → the invariant is no longer provable.
    let source = r#"
type Account {
  balance: Int
  active: Bool
}

intent Deposit(a: Account, amount: Int) {
  modifies *
  require amount > 0
  ensure a.balance' == a.balance + amount
  invariant a.active' == a.active
}
"#;
    let results = verify_file(source);
    let r = results.iter().find(|(n, _, _)| n == "Deposit").unwrap();
    assert!(
        matches!(r.2, VerifyResult::Failed { .. }),
        "wildcard modifies should disable frame semantics, got {:?}",
        r.2
    );
}

#[test]
fn reject_branch_vc_generated_and_verified() {
    // D3: `else reject` emits an extra VC — violated require + empty frame
    // must be consistent with invariants/safety.
    let source = r#"
type Account {
  balance: Int
}

safety NoNegative(a: Account) {
  invariant a.balance >= 0
}

intent Withdraw(a: Account, amount: Int) {
  require amount > 0
  require funds: a.balance >= amount else reject
  ensure a.balance' == a.balance - amount
}
"#;
    let results = verify_file(source);
    let reject = results
        .iter()
        .find(|(n, _, _)| n.contains("reject branch"))
        .expect("a reject-branch VC should be generated");
    assert!(
        reject.0.contains("Withdraw/funds"),
        "reject VC should carry the stable clause ID, got {}",
        reject.0
    );
    assert!(
        matches!(reject.2, VerifyResult::Verified),
        "reject branch (state unchanged) should be consistent with safety, got {:?}",
        reject.2
    );
}

#[test]
fn examples_may_pin_negative_values() {
    // Gravity, temperature, deltas and debts are all negative. Rejecting `-9`
    // as "not a literal" left whole domains with no example coverage at all:
    // the physics model's integration order had no machine guard, and swapping
    // semi-implicit for explicit Euler kept `check` green.
    let source = r#"
type Body {
  velocityY: Int
  posY: Int
}

intent Fall(b: Body, dt: Int) {
  modifies b.velocityY, b.posY
  require dt > 0
  ensure b.velocityY' == b.velocityY - 10 * dt
  ensure b.posY' == b.posY + b.velocityY' * dt
}

example Fall "one tick from rest" {
  given:  { b.velocityY: 0, b.posY: 100, dt: 1 }
  expect: { b.velocityY': -10, b.posY': 90 }
}

example Fall "already falling" {
  given:  { b.velocityY: -10, b.posY: 90, dt: 1 }
  expect: { b.velocityY': -20, b.posY': 70 }
}
"#;
    let prog = parse(source).expect("parse");
    let diags = check_program(&prog);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| matches!(d.level, DiagLevel::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "negative literals must survive typecheck, got {errors:?}"
    );

    for r in check_examples(&prog) {
        assert!(
            matches!(r.status, ExampleStatus::Consistent),
            "{:?}: {:?}",
            r.title,
            r.status
        );
    }
}

#[test]
fn a_wrong_negative_expectation_is_still_caught() {
    // The guard only means something if it can fail: the second example below
    // asserts the position of an *explicit* Euler step (posY + velocityY * dt
    // using the old velocity), which this semi-implicit model does not produce.
    let source = r#"
type Body {
  velocityY: Int
  posY: Int
}

intent Fall(b: Body, dt: Int) {
  modifies b.velocityY, b.posY
  require dt > 0
  ensure b.velocityY' == b.velocityY - 10 * dt
  ensure b.posY' == b.posY + b.velocityY' * dt
}

example Fall "explicit euler position" {
  given:  { b.velocityY: 0, b.posY: 100, dt: 1 }
  expect: { b.velocityY': -10, b.posY': 100 }
}
"#;
    let prog = parse(source).expect("parse");
    let results = check_examples(&prog);
    assert_eq!(results.len(), 1);
    assert!(
        !matches!(results[0].status, ExampleStatus::Consistent),
        "an example that contradicts the integration order must not pass, \
         got {:?}",
        results[0].status
    );
}

#[test]
fn unprimed_safety_constrains_the_post_state() {
    // An unprimed `safety` invariant used to be pushed as both an assumption
    // and the negated goal, which is unsatisfiable regardless of what the
    // operation does — so every such rule passed while proving nothing. The
    // whole point of writing one is to forbid operations that break it.
    let source = r#"
type Account {
  balance: Int
}

safety NonNegative(a: Account) {
  invariant a.balance >= 0
}

intent Overdraw(a: Account) {
  modifies a.balance
  ensure a.balance' == 0 - 1
}

intent Deposit(a: Account, amount: Int) {
  modifies a.balance
  require amount > 0
  ensure a.balance' == a.balance + amount
}
"#;
    let results = verify_file(source);

    let overdraw = results.iter().find(|(n, _, _)| n == "Overdraw").unwrap();
    assert!(
        matches!(overdraw.2, VerifyResult::Failed { .. }),
        "an operation that ends below zero must violate the safety rule, got {:?}",
        overdraw.2
    );

    let deposit = results.iter().find(|(n, _, _)| n == "Deposit").unwrap();
    assert!(
        matches!(deposit.2, VerifyResult::Verified),
        "an operation that preserves it must still verify, got {:?}",
        deposit.2
    );
}

#[test]
fn safety_rules_only_bind_intents_that_share_their_parameters() {
    // Safety parameters are free symbols matched by name. An intent that does
    // not declare `c: Counter` cannot reach `c.value`, and no frame equality
    // pins `c.value'` either — so demanding the proof would fail every
    // unrelated operation on a symbol it never touches.
    let source = r#"
type Counter {
  value: Int
}

type Flag {
  on: Bool
}

safety NonNegative(c: Counter) {
  invariant c.value >= 0
}

intent Unrelated(f: Flag) {
  modifies f.on
  ensure f.on' == true
}

intent Decrement(c: Counter) {
  modifies c.value
  ensure c.value' == c.value - 1
}
"#;
    let results = verify_file(source);

    let unrelated = results.iter().find(|(n, _, _)| n == "Unrelated").unwrap();
    assert!(
        matches!(unrelated.2, VerifyResult::Verified),
        "an intent with no Counter parameter is not governed by the rule, \
         got {:?}",
        unrelated.2
    );

    let decrement = results.iter().find(|(n, _, _)| n == "Decrement").unwrap();
    assert!(
        matches!(decrement.2, VerifyResult::Failed { .. }),
        "an intent that does hold the Counter must answer for it, got {:?}",
        decrement.2
    );
}

#[test]
fn invariants_relating_both_states_are_left_as_written() {
    // The counterpart to the rule above: an invariant that already mentions
    // the post-state is deliberately comparing the two, and priming the rest
    // of it would turn `a.balance' >= a.balance` into `a.balance' >=
    // a.balance'` — a tautology, i.e. the same vacuity from the other side.
    let source = r#"
type Account {
  balance: Int
}

intent Grow(a: Account, amount: Int) {
  modifies a.balance
  require amount > 0
  ensure a.balance' == a.balance + amount
  invariant a.balance' >= a.balance
}

intent Shrink(a: Account, amount: Int) {
  modifies a.balance
  require amount > 0
  ensure a.balance' == a.balance - amount
  invariant a.balance' >= a.balance
}
"#;
    let results = verify_file(source);

    let grow = results.iter().find(|(n, _, _)| n == "Grow").unwrap();
    assert!(
        matches!(grow.2, VerifyResult::Verified),
        "growing satisfies a monotonicity invariant, got {:?}",
        grow.2
    );

    let shrink = results.iter().find(|(n, _, _)| n == "Shrink").unwrap();
    assert!(
        matches!(shrink.2, VerifyResult::Failed { .. }),
        "shrinking violates it — the invariant must not collapse to a \
         tautology, got {:?}",
        shrink.2
    );
}

#[test]
fn scalar_parameters_are_not_state_and_must_not_be_primed() {
    // `amount` is an input, not state. Priming it would produce an
    // unconstrained `amount_prime` and fail a requirement that plainly holds.
    let source = r#"
type Account {
  balance: Int
}

intent Deposit(a: Account, amount: Int) {
  modifies a.balance
  require amount > 0
  ensure a.balance' == a.balance + amount
  invariant amount > 0
}
"#;
    let results = verify_file(source);
    let r = results.iter().find(|(n, _, _)| n == "Deposit").unwrap();
    assert!(
        matches!(r.2, VerifyResult::Verified),
        "an invariant over a scalar input should hold, got {:?}",
        r.2
    );
}

#[test]
fn smt_that_z3_cannot_parse_yields_no_verdict() {
    // `function` bodies are never encoded — vcgen emits the call as a bare
    // application of a symbol that was never declared, Z3 drops that whole
    // assertion and answers on what is left. Both of the resulting verdicts
    // were wrong and neither said so: with a goal to discharge it surfaced as
    // `Failed` with an empty counterexample, and without one as `Verified`.
    // Found by reverse-modeling a physics simulation, where four extracted
    // helpers made three intents fail for reasons no diagnostic could name.
    //
    // This test does not assert that functions are unsupported — it asserts
    // that a query the solver never received intact cannot produce a verdict.
    // Teaching the encoder to emit `define-fun` would keep it green.
    let source = r#"
type T {
  x: Int
}

function twice(n: Int) -> Int { n * 2 }

intent UseFn(t: T) {
  modifies t.x
  ensure t.x' == twice(t.x)
  invariant t.x' == t.x * 2
}

intent UseFnNoGoal(t: T) {
  modifies t.x
  ensure t.x' == twice(t.x)
}
"#;
    let results = verify_file(source);
    for name in ["UseFn", "UseFnNoGoal"] {
        let r = results.iter().find(|(n, _, _)| n == name).unwrap();
        assert!(
            matches!(r.2, VerifyResult::Error { .. }),
            "{name}: a dropped assertion must refuse a verdict, got {:?}",
            r.2
        );
    }
}

#[test]
fn well_formed_smt_still_reaches_the_solver_intact() {
    // Guards the assertion-count check itself: if Z3 ever ingested our output
    // as a different number of assertions than we wrote, every verdict in the
    // suite would turn into an error. Kept explicit so that failure mode is
    // named rather than diagnosed from a wall of unrelated red.
    let source = r#"
type Account {
  balance: Int
  active: Bool
}

intent Deposit(a: Account, amount: Int) {
  require amount > 0
  ensure a.balance' == a.balance + amount
  invariant a.balance' >= a.balance
  invariant a.active' == a.active
}
"#;
    let results = verify_file(source);
    let r = results.iter().find(|(n, _, _)| n == "Deposit").unwrap();
    assert!(
        matches!(r.2, VerifyResult::Verified),
        "multi-assertion VC should verify, got {:?}",
        r.2
    );
}

#[test]
fn inline_withdraw_with_guard_verified() {
    let source = r#"
type Account {
  balance: Int
}

intent Withdraw(acc: Account, amount: Int) {
  require amount > 0
  require acc.balance >= amount
  ensure acc.balance' == acc.balance - amount
  invariant acc.balance' >= 0
}
"#;
    let results = verify_file(source);
    let w = results.iter().find(|(n, _, _)| n == "Withdraw").unwrap();
    assert!(
        matches!(w.2, VerifyResult::Verified),
        "guarded Withdraw should verify"
    );
}
