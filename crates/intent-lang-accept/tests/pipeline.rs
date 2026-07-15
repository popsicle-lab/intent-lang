use std::path::Path;
use std::process::Command;

use intent_lang_accept::binding::load_binding;
use intent_lang_accept::codegen::generate;
use intent_lang_accept::report::{build_report, parse_junit, GateMode, TestResult};
use intent_lang_syntax::parse;

fn demo_dir() -> &'static Path {
    Path::new("../../examples/acceptance")
}

fn load_demo() -> (intent_lang_syntax::ast::Program, intent_lang_accept::binding::Binding) {
    let src = std::fs::read_to_string(demo_dir().join("transfer.intent")).unwrap();
    let prog = parse(&src).unwrap();
    let binding = load_binding(&demo_dir().join("transfer.intent.bind.toml")).unwrap();
    (prog, binding)
}

#[test]
fn binding_parses_and_validates() {
    let (_, binding) = load_demo();
    assert_eq!(binding.meta.adapter, "python-pytest");
    assert!(binding.types.contains_key("Account"));
    assert!(binding.state.contains_key("Account.balance"));
    let op = &binding.ops["TransferSafe"];
    assert_eq!(op.reject_signal.as_deref(), Some("raises"));
}

#[test]
fn codegen_embeds_clause_ids_and_covers_scenarios() {
    let (prog, binding) = load_demo();
    let gen = generate(&prog, &binding, "transfer.intent", "b.toml", ".");

    // Clause IDs must be embedded in assert messages (D9).
    assert!(gen.pytest_code.contains("clause TransferSafe/debit violated"));
    assert!(gen.pytest_code.contains("clause TransferSafe/credit violated"));
    // Frame assertion (D2).
    assert!(gen.pytest_code.contains("TransferSafe/frame"));
    // Reject tests for all three `else reject` requires (D3).
    assert!(gen.pytest_code.contains("pytest.raises(bank_demo.TransferError)"));
    let reject_tests = gen
        .manifest
        .tests
        .iter()
        .filter(|t| t.expect_reject)
        .count();
    assert_eq!(reject_tests, 3, "one reject test per `else reject` require");
    // Example tests come first (D5).
    assert!(gen.manifest.tests[0].scenario.starts_with("example"));
    // Goal rollup input present (D9).
    assert!(gen
        .manifest
        .goal_clauses
        .contains_key("转账绝不能凭空创造或销毁资金"));
    // No manual items in this fully-bound, quantifier-free demo.
    assert!(gen.manifest.manual_items.is_empty(), "{:?}", gen.manifest.manual_items);
}

#[test]
fn report_attributes_failure_to_blamed_clause_only() {
    let (prog, binding) = load_demo();
    let gen = generate(&prog, &binding, "transfer.intent", "b.toml", ".");

    // Simulate: happy test failed at the debit assert.
    let results: Vec<TestResult> = gen
        .manifest
        .tests
        .iter()
        .map(|t| TestResult {
            name: t.name.clone(),
            failure: if t.scenario == "happy" {
                Some(
                    "AssertionError: clause TransferSafe/debit violated: (sender.balance' == (sender.balance - amount))"
                        .to_string(),
                )
            } else {
                None
            },
        })
        .collect();

    let report = build_report(&gen.manifest, &results, GateMode::Strict);
    let debit = report
        .clauses
        .iter()
        .find(|c| c.id == "TransferSafe/debit")
        .unwrap();
    assert_eq!(debit.status, "failed");
    // credit passed in example tests, so it stays passed (evaluated there).
    let credit = report
        .clauses
        .iter()
        .find(|c| c.id == "TransferSafe/credit")
        .unwrap();
    assert_eq!(credit.status, "passed");
    assert_eq!(report.gate.verdict, "fail");
}

#[test]
fn junit_parser_handles_pass_and_failure() {
    let xml = r#"<?xml version="1.0"?><testsuites><testsuite>
<testcase classname="t" name="test_ok" time="0.001" />
<testcase classname="t" name="test_bad" time="0.002"><failure message="AssertionError: clause X/debit violated">trace</failure></testcase>
</testsuite></testsuites>"#;
    let results = parse_junit(xml);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "test_ok");
    assert!(results[0].failure.is_none());
    assert_eq!(results[1].name, "test_bad");
    assert!(results[1].failure.as_ref().unwrap().contains("X/debit"));
}

fn pytest_available() -> bool {
    Command::new("python3")
        .args(["-m", "pytest", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// M-A1 self-bootstrap acceptance criterion (RFC §5): with the seeded bug
/// the report must attribute the failure to the exact ensure clause ID
/// and the gate must fail. Skipped when python3/pytest is unavailable.
#[test]
fn seeded_bug_is_attributed_to_debit_clause() {
    if !pytest_available() {
        eprintln!("skipping: python3/pytest not available");
        return;
    }
    let (prog, binding) = load_demo();
    let source_dir = demo_dir().canonicalize().unwrap();
    let gen = generate(
        &prog,
        &binding,
        "transfer.intent",
        "b.toml",
        &source_dir.to_string_lossy(),
    );

    let out = std::env::temp_dir().join("intent-accept-selftest");
    std::fs::create_dir_all(&out).unwrap();
    let test_file = out.join("test_acceptance.py");
    std::fs::write(&test_file, &gen.pytest_code).unwrap();

    let junit = out.join("junit.xml");
    let status = Command::new("python3")
        .args(["-m", "pytest", "-q", "-p", "no:cacheprovider"])
        .arg(&test_file)
        .arg(format!("--junit-xml={}", junit.display()))
        .env("BANK_DEMO_BUGGY", "1")
        .current_dir(&out)
        .output()
        .unwrap();
    assert_eq!(status.status.code(), Some(1), "pytest should report failures");

    let xml = std::fs::read_to_string(&junit).unwrap();
    let results = parse_junit(&xml);
    let report = build_report(&gen.manifest, &results, GateMode::Strict);

    let debit = report
        .clauses
        .iter()
        .find(|c| c.id == "TransferSafe/debit")
        .expect("debit clause in report");
    assert_eq!(debit.status, "failed", "bug must be attributed to TransferSafe/debit");
    assert!(
        debit.detail.as_ref().unwrap().contains("TransferSafe/debit"),
        "failure detail should carry the clause ID"
    );
    // credit is only reached after debit passes → honest `blocked`, not green.
    let credit = report
        .clauses
        .iter()
        .find(|c| c.id == "TransferSafe/credit")
        .unwrap();
    assert_eq!(credit.status, "blocked");
    assert_eq!(report.gate.verdict, "fail");
}
