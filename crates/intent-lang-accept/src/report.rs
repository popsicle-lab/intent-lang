//! `intent.acceptance_report` (acceptance RFC 4.3, D9/D10):
//! merge pytest results back onto requirement clause IDs and goals.
//! The subject of the report is the requirement, never the test function.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::codegen::Manifest;

// ── JUnit parsing (minimal, pytest's junit-xml output) ───────

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub failure: Option<String>,
}

/// Parse pytest's JUnit XML just enough: testcase name + failure message.
pub fn parse_junit(xml: &str) -> Vec<TestResult> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<testcase") {
        rest = &rest[start..];
        let tag_end = match rest.find('>') {
            Some(i) => i,
            None => break,
        };
        let tag = &rest[..tag_end + 1];
        let name = extract_attr(tag, "name").unwrap_or_default();
        let self_closing = tag.ends_with("/>");

        let failure = if self_closing {
            None
        } else {
            let body_end = rest.find("</testcase>").unwrap_or(rest.len());
            let body = &rest[tag_end + 1..body_end];
            if let Some(fpos) = body.find("<failure").or_else(|| body.find("<error")) {
                let ftag_end = body[fpos..].find('>').map(|i| fpos + i + 1).unwrap_or(body.len());
                let ftag = &body[fpos..ftag_end];
                let msg = extract_attr(ftag, "message").unwrap_or_else(|| "test failed".to_string());
                Some(unescape_xml(&msg))
            } else {
                None
            }
        };

        out.push(TestResult { name, failure });
        rest = &rest[tag_end + 1..];
    }
    out
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    // Require a preceding delimiter so `name=` never matches `classname=`.
    let needle = format!(" {attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#10;", "\n")
        .replace("&amp;", "&")
}

// ── Report structures (RFC 4.3) ───────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AcceptanceReport {
    pub kind: String,
    pub file: String,
    pub binding: String,
    pub adapter: String,
    pub clauses: Vec<ClauseResult>,
    pub goals: Vec<GoalResult>,
    pub summary: Summary,
    pub gate: Gate,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClauseResult {
    pub id: String,
    /// passed | failed | manual-pending
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Number of scenarios that exercised this clause.
    pub scenarios: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalResult {
    pub name: String,
    pub machine: MachineCount,
    pub manual: ManualCount,
}

#[derive(Debug, Clone, Serialize)]
pub struct MachineCount {
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManualCount {
    pub confirmed: usize,
    pub pending: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub passed: usize,
    pub failed: usize,
    pub manual_pending: usize,
    pub tests_run: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gate {
    pub mode: String,
    pub verdict: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateMode {
    Strict,
    Lenient,
}

/// Run pytest on the generated file, producing JUnit XML in `out_dir`.
/// Returns Err only on infrastructure failure (pytest missing etc.);
/// test failures are normal data.
pub fn run_pytest(test_file: &Path, out_dir: &Path) -> Result<String, String> {
    let junit_path = out_dir.join("junit.xml");
    let status = Command::new("python3")
        .arg("-m")
        .arg("pytest")
        .arg(test_file)
        .arg("-q")
        .arg("--no-header")
        .arg(format!("--junit-xml={}", junit_path.display()))
        .arg("-p")
        .arg("no:cacheprovider")
        .current_dir(out_dir)
        .output()
        .map_err(|e| format!("cannot execute python3: {e}"))?;

    // pytest exit codes: 0 ok, 1 failures, others = infra errors.
    let code = status.status.code().unwrap_or(-1);
    if code != 0 && code != 1 {
        return Err(format!(
            "pytest infrastructure error (exit {code}):\n{}\n{}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        ));
    }
    std::fs::read_to_string(&junit_path).map_err(|e| format!("cannot read junit.xml: {e}"))
}

/// Merge test results back onto clause IDs (D9).
pub fn build_report(
    manifest: &Manifest,
    results: &[TestResult],
    gate_mode: GateMode,
) -> AcceptanceReport {
    let by_name: BTreeMap<&str, &TestResult> =
        results.iter().map(|r| (r.name.as_str(), r)).collect();

    // clause id → (scenarios, failures)
    let mut clause_runs: BTreeMap<String, (usize, Vec<(String, String)>)> = BTreeMap::new();
    for id in &manifest.machine_clause_ids {
        clause_runs.insert(id.clone(), (0, Vec::new()));
    }

    for t in &manifest.tests {
        let Some(res) = by_name.get(t.name.as_str()) else {
            // Test never ran (collection error) — count as failure on all
            // its clauses; silent loss would fake green.
            for id in &t.clause_ids {
                let entry = clause_runs.entry(id.clone()).or_default();
                entry.0 += 1;
                entry
                    .1
                    .push((t.scenario.clone(), "test did not run".to_string()));
            }
            continue;
        };
        match &res.failure {
            None => {
                for id in &t.clause_ids {
                    clause_runs.entry(id.clone()).or_default().0 += 1;
                }
            }
            Some(msg) => {
                // Attribute precisely: assert messages carry `clause <id>`.
                // Unblamed clauses in a failed test were never evaluated
                // (asserts run in order) — they count neither as passed
                // nor failed; if they end up with zero evaluated
                // scenarios they surface as `blocked` below.
                let blamed: Vec<&String> = t
                    .clause_ids
                    .iter()
                    .filter(|id| msg.contains(&format!("clause {id}")))
                    .collect();
                for id in &t.clause_ids {
                    let entry = clause_runs.entry((*id).clone()).or_default();
                    let is_blamed = blamed.is_empty() || blamed.iter().any(|b| *b == id);
                    if is_blamed {
                        entry.0 += 1;
                        entry.1.push((t.scenario.clone(), first_line(msg)));
                    }
                }
            }
        }
    }

    let mut clauses = Vec::new();
    let (mut passed, mut failed) = (0usize, 0usize);
    for (id, (scenarios, failures)) in &clause_runs {
        if failures.is_empty() {
            if *scenarios == 0 {
                // Never evaluated (earlier asserts in every test failed
                // first). Reporting "passed" here would be a fake green.
                clauses.push(ClauseResult {
                    id: id.clone(),
                    status: "blocked".to_string(),
                    detail: None,
                    scenario: None,
                    reason: Some(
                        "not evaluated — earlier assertions failed in every scenario".to_string(),
                    ),
                    scenarios: 0,
                });
                continue;
            }
            passed += 1;
            clauses.push(ClauseResult {
                id: id.clone(),
                status: "passed".to_string(),
                detail: None,
                scenario: None,
                reason: None,
                scenarios: *scenarios,
            });
        } else {
            failed += 1;
            clauses.push(ClauseResult {
                id: id.clone(),
                status: "failed".to_string(),
                detail: Some(failures[0].1.clone()),
                scenario: Some(failures[0].0.clone()),
                reason: None,
                scenarios: *scenarios,
            });
        }
    }
    for m in &manifest.manual_items {
        clauses.push(ClauseResult {
            id: m.clause_id.clone(),
            status: "manual-pending".to_string(),
            detail: None,
            scenario: None,
            reason: Some(m.reason.clone()),
            scenarios: 0,
        });
    }

    // Goal rollup via realized_by (D9).
    let status_of: BTreeMap<&str, &str> = clauses
        .iter()
        .map(|c| (c.id.as_str(), c.status.as_str()))
        .collect();
    let mut goals = Vec::new();
    for (goal, ids) in &manifest.goal_clauses {
        let mut mc = MachineCount {
            passed: 0,
            failed: 0,
            total: 0,
        };
        let mut pending = 0usize;
        for id in ids {
            match status_of.get(id.as_str()) {
                Some(&"passed") => {
                    mc.passed += 1;
                    mc.total += 1;
                }
                Some(&"failed") => {
                    mc.failed += 1;
                    mc.total += 1;
                }
                Some(&"blocked") => {
                    mc.total += 1;
                }
                Some(&"manual-pending") => pending += 1,
                // Clause exists but produced no test and no manual item
                // (e.g. plain require) — not an acceptance subject.
                _ => {}
            }
        }
        goals.push(GoalResult {
            name: goal.clone(),
            machine: mc,
            manual: ManualCount {
                confirmed: 0,
                pending,
            },
        });
    }

    let manual_pending = manifest.manual_items.len();
    let verdict = match gate_mode {
        GateMode::Strict => {
            if failed > 0 || manual_pending > 0 {
                "fail"
            } else {
                "pass"
            }
        }
        GateMode::Lenient => {
            if failed > 0 {
                "fail"
            } else if manual_pending > 0 {
                "pass-with-pending"
            } else {
                "pass"
            }
        }
    };

    AcceptanceReport {
        kind: "intent.acceptance_report".to_string(),
        file: manifest.intent_file.clone(),
        binding: manifest.binding_file.clone(),
        adapter: manifest.adapter.clone(),
        clauses,
        goals,
        summary: Summary {
            passed,
            failed,
            manual_pending,
            tests_run: results.len(),
        },
        gate: Gate {
            mode: match gate_mode {
                GateMode::Strict => "strict".to_string(),
                GateMode::Lenient => "lenient".to_string(),
            },
            verdict: verdict.to_string(),
        },
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).trim().to_string()
}
