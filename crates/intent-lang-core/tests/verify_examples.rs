use intent_lang_core::smt::{verify_vc, VerifyResult};
use intent_lang_core::vcgen::{generate_vcs, VcKind};
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
