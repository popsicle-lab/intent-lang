//! Requirements-modeling analyses (RFC: A4, B2, B3, B4, B5).
//!
//! These analyses operate on the AST and produce reports for humans
//! and downstream tools (popsicle Skills). All analyses are
//! deterministic: pure AST → text/JSON, no Z3 invocation.

use std::collections::{BTreeMap, HashMap, HashSet};

use intent_lang_syntax::ast::*;
use serde::Serialize;

// ── Pretty-printer for Expr (used in reports) ───────────────────

pub fn expr_to_text(e: &Spanned<Expr>) -> String {
    fn go(e: &Spanned<Expr>) -> String {
        match &e.node {
            Expr::IntLit(n) => n.to_string(),
            Expr::BoolLit(b) => b.to_string(),
            Expr::StringLit(s) => format!("\"{s}\""),
            Expr::Ident(n) => n.clone(),
            Expr::Prime(inner) => format!("{}'", go(inner)),
            Expr::FieldAccess(b, f) => format!("{}.{}", go(b), f),
            Expr::Index(b, i) => format!("{}[{}]", go(b), go(i)),
            Expr::BinOp(l, op, r) => format!("({} {} {})", go(l), op, go(r)),
            Expr::UnaryOp(UnaryOp::Not, o) => format!("!{}", go(o)),
            Expr::UnaryOp(UnaryOp::Neg, o) => format!("-{}", go(o)),
            Expr::IfThenElse(c, t, el) => {
                format!("if {} then {} else {}", go(c), go(t), go(el))
            }
            Expr::Forall(vs, body) => {
                let vars = vs
                    .iter()
                    .map(|v| format!("{}: {}", v.name, v.ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("forall {vars}, {}", go(body))
            }
            Expr::Exists(vs, body) => {
                let vars = vs
                    .iter()
                    .map(|v| format!("{}: {}", v.name, v.ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("exists {vars}, {}", go(body))
            }
            Expr::Call(n, args) => {
                let a = args.iter().map(go).collect::<Vec<_>>().join(", ");
                format!("{n}({a})")
            }
            Expr::Paren(inner) => format!("({})", go(inner)),
        }
    }
    go(e)
}

// ── @asis / @tobe annotation lookup (RFC A2) ────────────────────

pub fn intent_lifecycle(intent: &IntentDecl) -> Lifecycle {
    let has = |n: &str| intent.annotations.iter().any(|a| a.name == n);
    if has("asis") {
        Lifecycle::AsIs
    } else if has("tobe") {
        Lifecycle::ToBe
    } else {
        Lifecycle::Current
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    /// `@asis` — describes legacy code; excluded from primary consistency checks.
    AsIs,
    /// `@tobe` — describes target state; included in primary consistency checks.
    ToBe,
    /// No annotation — treated as current truth, included by default.
    Current,
}

// ── B2: Coverage analysis ──────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CoverageReport {
    pub coverages: Vec<CoverageScenarioReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CoverageScenarioReport {
    pub name: String,
    pub total_combinations: usize,
    pub covered: usize,
    pub uncovered: Vec<BTreeMap<String, String>>,
    pub witnessed_by: BTreeMap<String, Vec<String>>,
}

/// For each `coverage` declaration, enumerate the cartesian product of
/// declared dimension values. A combination is considered "covered" if
/// some safety invariant or intent clause syntactically mentions every
/// value name in the combination.
///
/// This is intentionally syntactic; it gives an honest first-pass
/// signal, not formal completeness. False negatives are expected and
/// acceptable per RFC §4 (under-engineering until dogfood).
pub fn coverage_report(prog: &Program) -> CoverageReport {
    let coverages: Vec<&CoverageDecl> = prog
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Coverage(c) => Some(c),
            _ => None,
        })
        .collect();

    let mut all_clauses: Vec<(String, String)> = Vec::new(); // (owner_name, text)
    for d in &prog.declarations {
        match &d.node {
            Declaration::Intent(i) => {
                for cl in &i.clauses {
                    all_clauses.push((i.name.clone(), expr_to_text(&cl.node.expr)));
                }
            }
            Declaration::Safety(s) => {
                for inv in &s.invariants {
                    all_clauses.push((s.name.clone(), expr_to_text(inv)));
                }
            }
            Declaration::Theorem(t) => {
                all_clauses.push((t.name.clone(), expr_to_text(&t.body)));
            }
            _ => {}
        }
    }

    let mut scenarios = Vec::new();
    for cov in coverages {
        // Cartesian product over dimensions.
        let dims: Vec<(String, Vec<String>)> = cov
            .dimensions
            .iter()
            .map(|d| {
                (
                    d.name.clone(),
                    d.values.iter().map(expr_to_text).collect::<Vec<_>>(),
                )
            })
            .collect();

        let combos = cartesian(&dims);
        let total = combos.len();
        let mut uncovered = Vec::new();
        let mut witnessed_by: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for combo in &combos {
            // Combo is covered if some clause text mentions every value.
            let key = combo
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",");

            let witness: Vec<String> = all_clauses
                .iter()
                .filter(|(_, text)| combo.iter().all(|(_, v)| text.contains(v)))
                .map(|(n, _)| n.clone())
                .collect();

            if witness.is_empty() {
                uncovered.push(combo.clone());
            } else {
                witnessed_by.insert(key, witness);
            }
        }

        scenarios.push(CoverageScenarioReport {
            name: cov.name.clone(),
            total_combinations: total,
            covered: total - uncovered.len(),
            uncovered,
            witnessed_by,
        });
    }

    CoverageReport {
        coverages: scenarios,
    }
}

fn cartesian(dims: &[(String, Vec<String>)]) -> Vec<BTreeMap<String, String>> {
    if dims.is_empty() {
        return vec![BTreeMap::new()];
    }
    let (head_name, head_vals) = &dims[0];
    let tail = cartesian(&dims[1..]);
    let mut out = Vec::with_capacity(head_vals.len() * tail.len());
    for v in head_vals {
        for t in &tail {
            let mut m = t.clone();
            m.insert(head_name.clone(), v.clone());
            out.push(m);
        }
    }
    out
}

// ── Clause index: stable IDs + executability (D4, D7) ──────────

/// Whether a clause can be turned into an executable acceptance check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Executability {
    /// Machine-checkable: quantifier-free, concrete pre/post assertion.
    Machine,
    /// Not machine-checkable (quantifiers — sampled semantics deferred
    /// until the `state` RFC lands). Goes to the manual checklist.
    Manual,
}

/// One clause with its stable ID (acceptance RFC 4.1, revised by D4).
#[derive(Debug, Clone, Serialize)]
pub struct ClauseInfo {
    /// `TransferSafe/debit` (labeled) or `TransferSafe/ensure[0]` (positional).
    pub id: String,
    pub owner: String,
    /// "require" | "ensure" | "invariant" | "safety"
    pub kind: String,
    pub label: Option<String>,
    pub text: String,
    /// D3: business rule — violation must reject with state unchanged.
    pub else_reject: bool,
    pub executability: Executability,
}

/// D7: static executability classification. Deterministic; quantified
/// clauses are Manual until `state` semantics exist. Program-aware:
/// a call to a function whose body contains a quantifier (`isSorted`,
/// `isPermutation`, ...) is also Manual.
pub fn expr_executability(prog: &Program, e: &Spanned<Expr>) -> Executability {
    fn has_quantifier(prog: &Program, e: &Spanned<Expr>) -> bool {
        match &e.node {
            Expr::Forall(..) | Expr::Exists(..) => true,
            Expr::BinOp(l, _, r) => has_quantifier(prog, l) || has_quantifier(prog, r),
            Expr::UnaryOp(_, o) => has_quantifier(prog, o),
            Expr::IfThenElse(c, t, el) => {
                has_quantifier(prog, c) || has_quantifier(prog, t) || has_quantifier(prog, el)
            }
            Expr::FieldAccess(b, _) | Expr::Prime(b) | Expr::Paren(b) => has_quantifier(prog, b),
            Expr::Index(b, i) => has_quantifier(prog, b) || has_quantifier(prog, i),
            Expr::Call(name, args) => {
                if args.iter().any(|a| has_quantifier(prog, a)) {
                    return true;
                }
                // Look through pure-function bodies.
                prog.declarations.iter().any(|d| match &d.node {
                    Declaration::Function(f) if &f.name == name => {
                        has_quantifier(prog, &f.body)
                    }
                    // Calls to intents (in theorems) or unknown imports:
                    // conservatively manual.
                    Declaration::Intent(i) if &i.name == name => true,
                    _ => false,
                })
            }
            _ => false,
        }
    }
    if has_quantifier(prog, e) {
        Executability::Manual
    } else {
        Executability::Machine
    }
}

/// Index every clause in the program with stable IDs.
pub fn clause_index(prog: &Program) -> Vec<ClauseInfo> {
    let mut out = Vec::new();
    for d in &prog.declarations {
        match &d.node {
            Declaration::Intent(i) => {
                let mut counters: BTreeMap<&'static str, usize> = BTreeMap::new();
                for cl in &i.clauses {
                    let kw = cl.node.kind.keyword();
                    let idx = counters.entry(kw).or_insert(0);
                    let id = cl.node.stable_id(&i.name, *idx);
                    *idx += 1;
                    out.push(ClauseInfo {
                        id,
                        owner: i.name.clone(),
                        kind: kw.to_string(),
                        label: cl.node.label.clone(),
                        text: expr_to_text(&cl.node.expr),
                        else_reject: cl.node.else_reject,
                        executability: expr_executability(prog, &cl.node.expr),
                    });
                }
            }
            Declaration::Safety(s) => {
                for (idx, inv) in s.invariants.iter().enumerate() {
                    out.push(ClauseInfo {
                        id: format!("{}/invariant[{idx}]", s.name),
                        owner: s.name.clone(),
                        kind: "safety".to_string(),
                        label: None,
                        text: expr_to_text(inv),
                        else_reject: false,
                        executability: expr_executability(prog, inv),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

// ── B3: Testspec generation ────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TestSpec {
    pub intents: Vec<IntentTestSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntentTestSpec {
    pub intent: String,
    pub lifecycle: Lifecycle,
    pub params: Vec<String>,
    pub scenarios: Vec<ScenarioRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioRow {
    /// "happy-path", "violates <clause-id>", "witnesses <clause-id>", ...
    pub label: String,
    /// Stable IDs of the clauses this scenario exercises.
    pub clause_ids: Vec<String>,
    /// D7: machine | manual — whether a test can be generated at all.
    pub executability: Executability,
    pub assumptions: Vec<String>,
    pub expected: String,
}

pub fn testspec(prog: &Program) -> TestSpec {
    let mut intents = Vec::new();
    let index = clause_index(prog);

    for d in &prog.declarations {
        if let Declaration::Intent(i) = &d.node {
            let mut scenarios = Vec::new();

            let clauses_of = |kind: &str| -> Vec<&ClauseInfo> {
                index
                    .iter()
                    .filter(|c| c.owner == i.name && c.kind == kind)
                    .collect()
            };
            let requires = clauses_of("require");
            let ensures = clauses_of("ensure");

            let all_machine = |cs: &[&ClauseInfo]| {
                cs.iter()
                    .all(|c| c.executability == Executability::Machine)
            };

            // Happy path: all requires hold; expect all ensures hold.
            scenarios.push(ScenarioRow {
                label: "happy-path".to_string(),
                clause_ids: ensures.iter().map(|c| c.id.clone()).collect(),
                executability: if all_machine(&requires) && all_machine(&ensures) {
                    Executability::Machine
                } else {
                    Executability::Manual
                },
                assumptions: requires.iter().map(|c| c.text.clone()).collect(),
                expected: if ensures.is_empty() {
                    "(no postconditions)".to_string()
                } else {
                    ensures
                        .iter()
                        .map(|c| c.text.clone())
                        .collect::<Vec<_>>()
                        .join(" && ")
                },
            });

            // Negative cases: violate each require in turn.
            // D3: `else reject` requires get a real assertion; unmarked
            // requires are caller contracts — behavior unspecified.
            for r in &requires {
                if r.else_reject {
                    scenarios.push(ScenarioRow {
                        label: format!("violates {}", r.id),
                        clause_ids: vec![r.id.clone()],
                        executability: r.executability,
                        assumptions: vec![format!("!({})", r.text)],
                        expected: "operation rejected && all state unchanged".to_string(),
                    });
                } else {
                    scenarios.push(ScenarioRow {
                        label: format!("violates {}", r.id),
                        clause_ids: vec![r.id.clone()],
                        executability: Executability::Manual,
                        assumptions: vec![format!("!({})", r.text)],
                        expected: "behavior unspecified — caller error".to_string(),
                    });
                }
            }

            // Boundary cases: each ensure interpreted as separate post.
            for e in &ensures {
                scenarios.push(ScenarioRow {
                    label: format!("witnesses {}", e.id),
                    clause_ids: vec![e.id.clone()],
                    executability: if all_machine(&requires)
                        && e.executability == Executability::Machine
                    {
                        Executability::Machine
                    } else {
                        Executability::Manual
                    },
                    assumptions: requires.iter().map(|c| c.text.clone()).collect(),
                    expected: e.text.clone(),
                });
            }

            intents.push(IntentTestSpec {
                intent: i.name.clone(),
                lifecycle: intent_lifecycle(i),
                params: i
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.ty))
                    .collect(),
                scenarios,
            });
        }
    }

    TestSpec { intents }
}

// ── A4 / B4: Diff & Impact ─────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    pub changes: Vec<Change>,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub potentially_breaking: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Change {
    Added {
        decl_kind: String,
        name: String,
    },
    Removed {
        decl_kind: String,
        name: String,
    },
    Modified {
        decl_kind: String,
        name: String,
        classification: ModificationKind,
        details: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModificationKind {
    /// Conditions weakened — accepts strict superset of states. Backward compatible.
    Loosened,
    /// Conditions strengthened — rejects states previously accepted. Potentially breaking.
    Tightened,
    /// Both directions or unclassifiable structural change.
    Reshaped,
}

pub fn diff(old: &Program, new: &Program) -> DiffReport {
    let old_idx = index_decls(old);
    let new_idx = index_decls(new);

    let mut changes = Vec::new();
    let (mut added, mut removed, mut modified, mut breaking) = (0, 0, 0, 0);

    for (k, sig) in &new_idx {
        if !old_idx.contains_key(k) {
            changes.push(Change::Added {
                decl_kind: k.0.clone(),
                name: k.1.clone(),
            });
            added += 1;
        } else {
            let old_sig = &old_idx[k];
            if old_sig != sig {
                let cls = classify(old_sig, sig);
                if cls == ModificationKind::Tightened || cls == ModificationKind::Reshaped {
                    breaking += 1;
                }
                changes.push(Change::Modified {
                    decl_kind: k.0.clone(),
                    name: k.1.clone(),
                    classification: cls,
                    details: structural_details(old_sig, sig),
                });
                modified += 1;
            }
        }
    }
    for k in old_idx.keys() {
        if !new_idx.contains_key(k) {
            changes.push(Change::Removed {
                decl_kind: k.0.clone(),
                name: k.1.clone(),
            });
            removed += 1;
            breaking += 1;
        }
    }

    DiffReport {
        changes,
        summary: DiffSummary {
            added,
            removed,
            modified,
            potentially_breaking: breaking,
        },
    }
}

/// Per-decl signature used for diffing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclSig {
    requires: Vec<String>,
    ensures: Vec<String>,
    invariants: Vec<String>,
    body: Vec<String>,
}

fn index_decls(prog: &Program) -> HashMap<(String, String), DeclSig> {
    let mut out = HashMap::new();
    for d in &prog.declarations {
        let (k, sig) = match &d.node {
            Declaration::Intent(i) => {
                let mut r = vec![];
                let mut e = vec![];
                let mut iv = vec![];
                for cl in &i.clauses {
                    let text = expr_to_text(&cl.node.expr);
                    match cl.node.kind {
                        ClauseKind::Require => r.push(text),
                        ClauseKind::Ensure => e.push(text),
                        ClauseKind::Invariant => iv.push(text),
                    }
                }
                (
                    ("intent".to_string(), i.name.clone()),
                    DeclSig {
                        requires: r,
                        ensures: e,
                        invariants: iv,
                        body: vec![],
                    },
                )
            }
            Declaration::Safety(s) => (
                ("safety".to_string(), s.name.clone()),
                DeclSig {
                    requires: vec![],
                    ensures: vec![],
                    invariants: s.invariants.iter().map(expr_to_text).collect(),
                    body: vec![],
                },
            ),
            Declaration::Theorem(t) => (
                ("theorem".to_string(), t.name.clone()),
                DeclSig {
                    requires: vec![],
                    ensures: vec![],
                    invariants: vec![],
                    body: vec![expr_to_text(&t.body)],
                },
            ),
            Declaration::Goal(g) => (
                ("goal".to_string(), g.name.clone()),
                DeclSig {
                    requires: vec![],
                    ensures: vec![],
                    invariants: g.realized_by.clone(),
                    body: vec![
                        g.rationale.clone().unwrap_or_default(),
                        g.measure.clone().unwrap_or_default(),
                    ],
                },
            ),
            Declaration::Coverage(c) => {
                let body: Vec<String> = c
                    .dimensions
                    .iter()
                    .map(|d| {
                        format!(
                            "{}={}",
                            d.name,
                            d.values
                                .iter()
                                .map(expr_to_text)
                                .collect::<Vec<_>>()
                                .join("|")
                        )
                    })
                    .collect();
                (
                    ("coverage".to_string(), c.name.clone()),
                    DeclSig {
                        requires: vec![],
                        ensures: vec![],
                        invariants: vec![],
                        body,
                    },
                )
            }
            Declaration::Type(t) => (
                ("type".to_string(), t.name.clone()),
                DeclSig {
                    requires: vec![],
                    ensures: vec![],
                    invariants: vec![],
                    body: t
                        .fields
                        .iter()
                        .map(|f| format!("{}: {}", f.name, f.ty))
                        .collect(),
                },
            ),
            Declaration::Enum(e) => (
                ("enum".to_string(), e.name.clone()),
                DeclSig {
                    requires: vec![],
                    ensures: vec![],
                    invariants: vec![],
                    body: e.variants.clone(),
                },
            ),
            _ => continue,
        };
        out.insert(k, sig);
    }
    out
}

fn classify(old: &DeclSig, new: &DeclSig) -> ModificationKind {
    // Heuristic: if requires got *fewer/weaker* (subset of old) and
    // ensures/invariants stayed/strengthened (superset of old), → Loosened.
    // If requires grew (superset) or ensures shrank, → Tightened.
    // Otherwise Reshaped.
    let old_req: HashSet<_> = old.requires.iter().collect();
    let new_req: HashSet<_> = new.requires.iter().collect();
    let old_ens: HashSet<_> = old.ensures.iter().collect();
    let new_ens: HashSet<_> = new.ensures.iter().collect();

    let req_subset = new_req.is_subset(&old_req);
    let req_superset = new_req.is_superset(&old_req);
    let ens_superset = new_ens.is_superset(&old_ens);
    let ens_subset = new_ens.is_subset(&old_ens);

    if req_subset && ens_superset && (new_req != old_req || new_ens != old_ens) {
        ModificationKind::Loosened
    } else if req_superset && ens_subset && (new_req != old_req || new_ens != old_ens) {
        ModificationKind::Tightened
    } else {
        ModificationKind::Reshaped
    }
}

fn structural_details(old: &DeclSig, new: &DeclSig) -> Vec<String> {
    let mut out = Vec::new();
    diff_set("require", &old.requires, &new.requires, &mut out);
    diff_set("ensure", &old.ensures, &new.ensures, &mut out);
    diff_set("invariant", &old.invariants, &new.invariants, &mut out);
    diff_set("body", &old.body, &new.body, &mut out);
    out
}

fn diff_set(label: &str, old: &[String], new: &[String], out: &mut Vec<String>) {
    let o: HashSet<_> = old.iter().cloned().collect();
    let n: HashSet<_> = new.iter().cloned().collect();
    for s in n.difference(&o) {
        out.push(format!("+ {label}: {s}"));
    }
    for s in o.difference(&n) {
        out.push(format!("- {label}: {s}"));
    }
}

/// RFC B4: impact analysis. Walk goal.realized_by + coverage witnesses
/// to find downstream effects of a diff.
#[derive(Debug, Clone, Serialize)]
pub struct ImpactReport {
    pub diff: DiffReport,
    pub affected_goals: Vec<String>,
    pub affected_coverages: Vec<String>,
}

pub fn impact(old: &Program, new: &Program) -> ImpactReport {
    let d = diff(old, new);

    // For each modified/removed intent or safety, find goals referencing it.
    let mut touched_names: HashSet<String> = HashSet::new();
    for c in &d.changes {
        match c {
            Change::Modified { name, .. } | Change::Removed { name, .. } => {
                touched_names.insert(name.clone());
            }
            _ => {}
        }
    }

    let mut affected_goals = Vec::new();
    let mut affected_coverages = Vec::new();
    for decl in &new.declarations {
        match &decl.node {
            Declaration::Goal(g) => {
                if g.realized_by.iter().any(|n| touched_names.contains(n)) {
                    affected_goals.push(g.name.clone());
                }
            }
            Declaration::Coverage(c) => {
                // Coverage is affected if any touched intent had clauses that
                // reference any dimension value.
                let dim_values: HashSet<String> = c
                    .dimensions
                    .iter()
                    .flat_map(|d| d.values.iter().map(expr_to_text))
                    .collect();
                let mut touched_clauses_text: Vec<String> = Vec::new();
                for old_d in &old.declarations {
                    if let Declaration::Intent(i) = &old_d.node {
                        if touched_names.contains(&i.name) {
                            for cl in &i.clauses {
                                touched_clauses_text.push(expr_to_text(&cl.node.expr));
                            }
                        }
                    }
                }
                if dim_values
                    .iter()
                    .any(|v| touched_clauses_text.iter().any(|t| t.contains(v)))
                {
                    affected_coverages.push(c.name.clone());
                }
            }
            _ => {}
        }
    }

    ImpactReport {
        diff: d,
        affected_goals,
        affected_coverages,
    }
}

// ── B5: Explain ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ExplainReport {
    pub target: String,
    pub kind: String,
    pub plain_english: String,
    pub satisfying_example: Option<String>,
    pub violating_example: Option<String>,
    pub clauses: Vec<ClauseExplanation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClauseExplanation {
    pub kind: String,
    pub formal: String,
    pub natural: String,
}

/// Generate a plain-English explanation of an intent/safety/goal.
/// This is template-based — no LLM call. The output is meant to be
/// human-reviewed and is consumed by popsicle's adversarial-reviewer Skill.
pub fn explain(prog: &Program, name: &str) -> Option<ExplainReport> {
    for d in &prog.declarations {
        match &d.node {
            Declaration::Intent(i) if i.name == name => {
                let mut clauses = Vec::new();
                for cl in &i.clauses {
                    let kind = match cl.node.kind {
                        ClauseKind::Require => {
                            if cl.node.else_reject {
                                "precondition (business rule: violation rejects)"
                            } else {
                                "precondition"
                            }
                        }
                        ClauseKind::Ensure => "postcondition",
                        ClauseKind::Invariant => "invariant",
                    };
                    let formal = expr_to_text(&cl.node.expr);
                    let natural = naturalize(&formal);
                    clauses.push(ClauseExplanation {
                        kind: kind.to_string(),
                        formal,
                        natural,
                    });
                }
                let plain = format!(
                    "Intent `{}` describes the operation that takes {} and constrains its behavior via {} clause(s).",
                    i.name,
                    i.params
                        .iter()
                        .map(|p| format!("{}: {}", p.name, p.ty))
                        .collect::<Vec<_>>()
                        .join(", "),
                    clauses.len()
                );
                return Some(ExplainReport {
                    target: i.name.clone(),
                    kind: "intent".to_string(),
                    plain_english: plain,
                    // Placeholders: real witnesses require Z3 model — TODO Phase 2.
                    satisfying_example: Some(
                        "(run `intent check` and inspect Z3 model — automated witnesses pending)"
                            .to_string(),
                    ),
                    violating_example: Some(
                        "(violation witness requires running with --counterexample flag)"
                            .to_string(),
                    ),
                    clauses,
                });
            }
            Declaration::Safety(s) if s.name == name => {
                let mut clauses = Vec::new();
                for inv in &s.invariants {
                    let formal = expr_to_text(inv);
                    let natural = naturalize(&formal);
                    clauses.push(ClauseExplanation {
                        kind: "invariant".to_string(),
                        formal,
                        natural,
                    });
                }
                let plain = format!(
                    "Safety rule `{}` declares {} invariant(s) that must hold across every intent that touches its parameters.",
                    s.name,
                    clauses.len()
                );
                return Some(ExplainReport {
                    target: s.name.clone(),
                    kind: "safety".to_string(),
                    plain_english: plain,
                    satisfying_example: None,
                    violating_example: None,
                    clauses,
                });
            }
            Declaration::Goal(g) if g.name == name => {
                let plain = format!(
                    "Goal \"{}\" — rationale: {}; measured by: {}; realized by: {}.",
                    g.name,
                    g.rationale.clone().unwrap_or_else(|| "(none)".to_string()),
                    g.measure.clone().unwrap_or_else(|| "(none)".to_string()),
                    if g.realized_by.is_empty() {
                        "(unlinked)".to_string()
                    } else {
                        g.realized_by.join(", ")
                    }
                );
                return Some(ExplainReport {
                    target: g.name.clone(),
                    kind: "goal".to_string(),
                    plain_english: plain,
                    satisfying_example: None,
                    violating_example: None,
                    clauses: vec![],
                });
            }
            _ => {}
        }
    }
    None
}

/// Very simple naturalizer — replaces operators with English-like phrases.
/// It is intentionally crude: humans review and refine. This is *not* an LLM.
fn naturalize(formal: &str) -> String {
    formal
        .replace("==>", "implies")
        .replace("&&", "and")
        .replace("||", "or")
        .replace("==", "equals")
        .replace("!=", "is not equal to")
        .replace(">=", "is at least")
        .replace("<=", "is at most")
        .replace("forall", "for every")
        .replace("exists", "there exists")
}
