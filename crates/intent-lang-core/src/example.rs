//! D5 (rfc-modeling-integrity): `example` block checking.
//!
//! An example pins the author's *intent* with concrete values. Z3 checks
//! that the values are consistent with every clause — the only machine
//! check that can catch formalization drift ("the formula is self-
//! consistent but not what the author meant").

use intent_lang_syntax::ast::*;
use serde::Serialize;

use crate::analysis::{clause_index, expr_to_text, ClauseInfo};
use crate::smt::{solve_constraints, SatOutcome};
use crate::vcgen::{frame_equalities_for, SmtDecl, VcKind, VerificationCondition};

#[derive(Debug, Clone, Serialize)]
pub struct ExampleResult {
    pub intent: String,
    pub title: Option<String>,
    pub status: ExampleStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExampleStatus {
    /// The example satisfies every clause of its intent.
    Consistent,
    /// The example contradicts a specific clause — formalization drift.
    Violates { clause_id: String, clause: String },
    /// Individually fine, but the example + full clause set is unsat.
    Inconsistent,
    /// Z3 could not decide.
    Unknown { reason: String },
}

/// Turn an example binding into an equality constraint `path == value`.
fn binding_eq(b: &ExampleBinding) -> Spanned<Expr> {
    let span = b.path.span.merge(&b.value.span);
    Spanned::new(
        Expr::BinOp(
            Box::new(b.path.clone()),
            BinOp::Eq,
            Box::new(b.value.clone()),
        ),
        span,
    )
}

/// Check every `example` declaration in the program.
pub fn check_examples(prog: &Program) -> Vec<ExampleResult> {
    let index = clause_index(prog);
    let mut results = Vec::new();

    for d in &prog.declarations {
        let Declaration::Example(ex) = &d.node else {
            continue;
        };
        let Some(intent) = prog.declarations.iter().find_map(|d| match &d.node {
            Declaration::Intent(i) if i.name == ex.intent => Some(i),
            _ => None,
        }) else {
            // Unknown intent is already an E0008 typecheck error; skip here.
            continue;
        };

        results.push(check_one(prog, &index, ex, intent));
    }

    results
}

fn check_one(
    prog: &Program,
    index: &[ClauseInfo],
    ex: &ExampleDecl,
    intent: &IntentDecl,
) -> ExampleResult {
    // Base constraints: given + expect equalities + frame equalities (D2 —
    // post-state fields the example doesn't mention follow frame semantics).
    let mut base: Vec<Spanned<Expr>> = Vec::new();
    for b in ex.given.iter().chain(ex.expect.iter()) {
        base.push(binding_eq(b));
    }
    base.extend(frame_equalities_for(prog, intent));

    let declarations: Vec<SmtDecl> = intent
        .params
        .iter()
        .map(|p| SmtDecl::DeclareConst(p.name.clone(), p.ty.clone()))
        .collect();
    let vc_shell = VerificationCondition {
        name: format!("example {}", ex.intent),
        kind: VcKind::Intent,
        declarations,
        assumes: Vec::new(),
        goals: Vec::new(),
        safety_rules: Vec::new(),
        unsupported: None,
    };

    // Per-clause check: pinpoint which clause the example contradicts.
    let intent_clauses: Vec<&ClauseInfo> =
        index.iter().filter(|c| c.owner == intent.name).collect();
    let mut clause_exprs: Vec<(String, String, Spanned<Expr>)> = Vec::new();
    {
        // Recover expressions in the same order clause_index produced IDs.
        let mut i = 0usize;
        for cl in &intent.clauses {
            let info = intent_clauses[i];
            clause_exprs.push((info.id.clone(), info.text.clone(), cl.node.expr.clone()));
            // Invariants must hold in the pre-state too.
            if cl.node.kind == ClauseKind::Invariant {
                clause_exprs.push((
                    format!("{} (pre-state)", info.id),
                    info.text.clone(),
                    crate::vcgen::unprime_expr(&cl.node.expr),
                ));
            }
            i += 1;
        }
    }

    for (id, text, expr) in &clause_exprs {
        let mut constraints = base.clone();
        constraints.push(expr.clone());
        match solve_constraints(&vc_shell, prog, &constraints) {
            SatOutcome::Sat { .. } => {}
            SatOutcome::Unsat => {
                return ExampleResult {
                    intent: ex.intent.clone(),
                    title: ex.title.clone(),
                    status: ExampleStatus::Violates {
                        clause_id: id.clone(),
                        clause: text.clone(),
                    },
                };
            }
            SatOutcome::Unknown { reason } => {
                return ExampleResult {
                    intent: ex.intent.clone(),
                    title: ex.title.clone(),
                    status: ExampleStatus::Unknown { reason },
                };
            }
            // A query that lost assertions can only come back weaker, so the
            // risk here is a `Violates` we failed to see — never report the
            // example as consistent on that basis.
            SatOutcome::Error { message } => {
                return ExampleResult {
                    intent: ex.intent.clone(),
                    title: ex.title.clone(),
                    status: ExampleStatus::Unknown { reason: message },
                };
            }
        }
    }

    // Combined check: bindings + all clauses at once.
    let mut constraints = base.clone();
    for (_, _, expr) in &clause_exprs {
        constraints.push(expr.clone());
    }
    match solve_constraints(&vc_shell, prog, &constraints) {
        SatOutcome::Sat { .. } => ExampleResult {
            intent: ex.intent.clone(),
            title: ex.title.clone(),
            status: ExampleStatus::Consistent,
        },
        SatOutcome::Unsat => ExampleResult {
            intent: ex.intent.clone(),
            title: ex.title.clone(),
            status: ExampleStatus::Inconsistent,
        },
        SatOutcome::Unknown { reason } => ExampleResult {
            intent: ex.intent.clone(),
            title: ex.title.clone(),
            status: ExampleStatus::Unknown { reason },
        },
        SatOutcome::Error { message } => ExampleResult {
            intent: ex.intent.clone(),
            title: ex.title.clone(),
            status: ExampleStatus::Unknown { reason: message },
        },
    }
}

/// Text of an example's given bindings — used in reports.
pub fn example_summary(ex: &ExampleDecl) -> String {
    let given: Vec<String> = ex
        .given
        .iter()
        .map(|b| format!("{}={}", expr_to_text(&b.path), expr_to_text(&b.value)))
        .collect();
    given.join(", ")
}
