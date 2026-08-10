//! D8 (acceptance RFC): witness solving — concrete test inputs derived
//! from the same Z3 that checks requirement consistency.
//!
//! - happy witness: a model of require ∧ ensure ∧ invariant ∧ frame;
//! - negative witness per `require`: a model of ¬r ∧ (other requires) ∧
//!   pre-state invariants/safety (the pre-state must still be a valid
//!   world — only the violated rule distinguishes the scenario);
//! - boundary witness per closed comparison (`>=`/`<=`): the equality
//!   point, solved not guessed.

use std::collections::BTreeMap;

use intent_lang_syntax::ast::*;
use serde::Serialize;

use crate::analysis::{clause_index, Executability};
use crate::smt::{solve_constraints, SatOutcome};
use crate::vcgen::{frame_equalities_for, unprime_expr, SmtDecl, VcKind, VerificationCondition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WitnessKind {
    Happy,
    ViolatesRequire,
    Boundary,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioWitness {
    /// e.g. "happy", "violates TransferSafe/funds", "boundary TransferSafe/funds"
    pub label: String,
    pub kind: WitnessKind,
    /// Clause IDs this scenario exercises.
    pub clause_ids: Vec<String>,
    /// D3: whether the scenario expects rejection + unchanged state.
    pub expect_reject: bool,
    /// Pre-state / parameter assignment: dotted path → literal text.
    pub values: BTreeMap<String, String>,
    /// Post-state assignment from the model (reference only; the real
    /// oracle is the clause expression evaluated over runtime reads).
    pub expected: BTreeMap<String, String>,
}

/// All solved witnesses for one intent.
#[derive(Debug, Clone, Serialize)]
pub struct IntentWitnesses {
    pub intent: String,
    pub scenarios: Vec<ScenarioWitness>,
    /// Scenarios that could not be solved (unsat constraint combination
    /// is normal for boundaries; unknown means Z3 gave up).
    pub unsolved: Vec<String>,
}

/// Map flattened SMT constant names back to dotted paths, using the
/// program's type declarations (never naive underscore replacement).
fn reverse_name_map(prog: &Program, intent: &IntentDecl) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for p in &intent.params {
        let ty_name = match &p.ty {
            TypeExpr::Named(n) => n.clone(),
            TypeExpr::Qualified(m, n) => format!("{m}.{n}"),
            TypeExpr::Generic(..) => {
                map.insert(p.name.clone(), p.name.clone());
                continue;
            }
        };
        let struct_fields = prog.declarations.iter().find_map(|d| match &d.node {
            Declaration::Type(t) if t.name == ty_name => Some(&t.fields),
            _ => None,
        });
        match struct_fields {
            Some(fields) => {
                for f in fields {
                    map.insert(format!("{}_{}", p.name, f.name), format!("{}.{}", p.name, f.name));
                }
            }
            None => {
                map.insert(p.name.clone(), p.name.clone());
            }
        }
    }
    map
}

fn split_model(
    model: Vec<(String, String)>,
    names: &BTreeMap<String, String>,
) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let mut pre = BTreeMap::new();
    let mut post = BTreeMap::new();
    for (flat, value) in model {
        if let Some(base) = flat.strip_suffix("_prime") {
            if let Some(path) = names.get(base) {
                post.insert(format!("{path}'"), value);
            }
        } else if let Some(path) = names.get(&flat) {
            pre.insert(path.clone(), value);
        }
    }
    (pre, post)
}

/// Replace closed comparisons (`>=`, `<=`) at the equality point.
/// Returns None if the expression contains no closed comparison.
fn tighten_to_equality(e: &Spanned<Expr>) -> Option<Spanned<Expr>> {
    match &e.node {
        Expr::BinOp(l, op, r) if matches!(op, BinOp::Ge | BinOp::Le) => Some(Spanned::new(
            Expr::BinOp(l.clone(), BinOp::Eq, r.clone()),
            e.span.clone(),
        )),
        Expr::BinOp(l, op, r) => {
            // Try either side of logical connectives.
            if matches!(op, BinOp::And | BinOp::Or | BinOp::Implies) {
                if let Some(nl) = tighten_to_equality(l) {
                    return Some(Spanned::new(
                        Expr::BinOp(Box::new(nl), *op, r.clone()),
                        e.span.clone(),
                    ));
                }
                if let Some(nr) = tighten_to_equality(r) {
                    return Some(Spanned::new(
                        Expr::BinOp(l.clone(), *op, Box::new(nr)),
                        e.span.clone(),
                    ));
                }
            }
            None
        }
        Expr::Paren(inner) => tighten_to_equality(inner).map(|ne| {
            Spanned::new(Expr::Paren(Box::new(ne)), e.span.clone())
        }),
        _ => None,
    }
}

fn negate(e: &Spanned<Expr>) -> Spanned<Expr> {
    Spanned::new(
        Expr::UnaryOp(
            UnaryOp::Not,
            Box::new(Spanned::new(
                Expr::Paren(Box::new(e.clone())),
                e.span.clone(),
            )),
        ),
        e.span.clone(),
    )
}

/// Solve witnesses for every intent in the program.
/// Only machine-checkable (quantifier-free) constraint sets are attempted.
pub fn program_witnesses(prog: &Program) -> Vec<IntentWitnesses> {
    let index = clause_index(prog);
    let mut out = Vec::new();

    for d in &prog.declarations {
        let Declaration::Intent(intent) = &d.node else {
            continue;
        };

        let declarations: Vec<SmtDecl> = intent
            .params
            .iter()
            .map(|p| SmtDecl::DeclareConst(p.name.clone(), p.ty.clone()))
            .collect();
        let shell = VerificationCondition {
            name: format!("witness {}", intent.name),
            kind: VcKind::Intent,
            declarations,
            assumes: Vec::new(),
            goals: Vec::new(),
            safety_rules: Vec::new(),
            unsupported: None,
        };
        let names = reverse_name_map(prog, intent);

        // Partition this intent's clauses.
        let infos: Vec<_> = index.iter().filter(|c| c.owner == intent.name).collect();
        let machine_only = |kind: &str| -> bool {
            infos
                .iter()
                .filter(|c| c.kind == kind)
                .all(|c| c.executability == Executability::Machine)
        };

        let requires: Vec<(&crate::analysis::ClauseInfo, Spanned<Expr>)> = {
            let mut v = Vec::new();
            let mut i = 0usize;
            for cl in &intent.clauses {
                let info = infos[i];
                i += 1;
                if cl.node.kind == ClauseKind::Require {
                    v.push((info, cl.node.expr.clone()));
                }
            }
            v
        };
        let ensures: Vec<Spanned<Expr>> = intent
            .clauses
            .iter()
            .filter(|c| c.node.kind == ClauseKind::Ensure)
            .map(|c| c.node.expr.clone())
            .collect();
        let invariants: Vec<Spanned<Expr>> = intent
            .clauses
            .iter()
            .filter(|c| c.node.kind == ClauseKind::Invariant)
            .map(|c| c.node.expr.clone())
            .collect();
        let ensure_ids: Vec<String> = infos
            .iter()
            .filter(|c| c.kind == "ensure")
            .map(|c| c.id.clone())
            .collect();

        let frame = frame_equalities_for(prog, intent);

        let mut scenarios = Vec::new();
        let mut unsolved = Vec::new();

        // If any require/ensure is quantified, skip witness generation for
        // this intent entirely (manual item per D7).
        if !(machine_only("require") && machine_only("ensure") && machine_only("invariant")) {
            out.push(IntentWitnesses {
                intent: intent.name.clone(),
                scenarios,
                unsolved: vec!["quantified clauses — manual acceptance item (D7)".to_string()],
            });
            continue;
        }

        // ── Happy witness ─────────────────────────────────
        {
            let mut cs: Vec<Spanned<Expr>> = Vec::new();
            cs.extend(requires.iter().map(|(_, e)| e.clone()));
            cs.extend(ensures.iter().cloned());
            for inv in &invariants {
                cs.push(unprime_expr(inv));
                cs.push(inv.clone());
            }
            cs.extend(frame.iter().cloned());
            match solve_constraints(&shell, prog, &cs) {
                SatOutcome::Sat { model } => {
                    let (values, expected) = split_model(model, &names);
                    scenarios.push(ScenarioWitness {
                        label: "happy".to_string(),
                        kind: WitnessKind::Happy,
                        clause_ids: ensure_ids.clone(),
                        expect_reject: false,
                        values,
                        expected,
                    });
                }
                SatOutcome::Unsat => unsolved.push(
                    "happy: clauses unsatisfiable (should have been caught by V0020)".to_string(),
                ),
                SatOutcome::Unknown { reason } => unsolved.push(format!("happy: {reason}")),
                SatOutcome::Error { message } => unsolved.push(format!("happy: {message}")),
            }
        }

        // ── Negative witnesses: violate each require ──────
        for (info, req) in &requires {
            let mut cs: Vec<Spanned<Expr>> = Vec::new();
            cs.push(negate(req));
            for (other_info, other) in &requires {
                if other_info.id != info.id {
                    cs.push(other.clone());
                }
            }
            // The pre-state must still be a valid world.
            for inv in &invariants {
                cs.push(unprime_expr(inv));
            }
            match solve_constraints(&shell, prog, &cs) {
                SatOutcome::Sat { model } => {
                    let (values, _) = split_model(model, &names);
                    scenarios.push(ScenarioWitness {
                        label: format!("violates {}", info.id),
                        kind: WitnessKind::ViolatesRequire,
                        clause_ids: vec![info.id.clone()],
                        expect_reject: info.else_reject,
                        values,
                        expected: BTreeMap::new(),
                    });
                }
                SatOutcome::Unsat => unsolved.push(format!(
                    "violates {}: unreachable (implied by other requires)",
                    info.id
                )),
                SatOutcome::Unknown { reason } => {
                    unsolved.push(format!("violates {}: {reason}", info.id))
                }
                SatOutcome::Error { message } => {
                    unsolved.push(format!("violates {}: {message}", info.id))
                }
            }
        }

        // ── Boundary witnesses: closed comparisons at equality ──
        for (info, req) in &requires {
            let Some(tightened) = tighten_to_equality(req) else {
                continue;
            };
            let mut cs: Vec<Spanned<Expr>> = Vec::new();
            cs.push(tightened);
            for (other_info, other) in &requires {
                if other_info.id != info.id {
                    cs.push(other.clone());
                }
            }
            cs.extend(ensures.iter().cloned());
            for inv in &invariants {
                cs.push(unprime_expr(inv));
                cs.push(inv.clone());
            }
            cs.extend(frame.iter().cloned());
            match solve_constraints(&shell, prog, &cs) {
                SatOutcome::Sat { model } => {
                    let (values, expected) = split_model(model, &names);
                    scenarios.push(ScenarioWitness {
                        label: format!("boundary {}", info.id),
                        kind: WitnessKind::Boundary,
                        clause_ids: ensure_ids.clone(),
                        expect_reject: false,
                        values,
                        expected,
                    });
                }
                // Unsat boundary is normal (equality point excluded by
                // other constraints) — not an error, just skip.
                SatOutcome::Unsat => {}
                SatOutcome::Unknown { reason } => {
                    unsolved.push(format!("boundary {}: {reason}", info.id))
                }
                SatOutcome::Error { message } => {
                    unsolved.push(format!("boundary {}: {message}", info.id))
                }
            }
        }

        out.push(IntentWitnesses {
            intent: intent.name.clone(),
            scenarios,
            unsolved,
        });
    }

    out
}
