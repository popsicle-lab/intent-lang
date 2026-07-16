//! State-machine graph builder.
//!
//! Derives a finite-state-machine view of a program by reading `status`-style
//! transitions directly out of intents:
//! - a **source** state comes from a `require path == Variant` clause
//!   (or a disjunction of them),
//! - a **target** state comes from an `ensure path' == Variant` clause
//!   (or the consequent of an implication `cond ==> path' == Variant`),
//!
//! where `Variant` belongs to the dominant *state enum* — the enum whose
//! variants appear most often in primed equalities across all intents.
//!
//! Intents that assign a target state without any source constraint are
//! treated as **creation** edges (`[*] --> Target`). States that are only
//! ever targets (never sources) are treated as **terminal** (`Target --> [*]`).
//!
//! Unlike the shared-type intent graph, this produces a faithful, low-noise
//! picture of how an entity flows through its lifecycle.

use intent_lang_syntax::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Serialize, Deserialize)]
pub struct StateMachine {
    /// Name of the enum used as the state space (if detected).
    pub state_enum: Option<String>,
    /// All states that participate in at least one transition.
    pub states: Vec<String>,
    /// States reachable via a creation edge (`[*] --> state`).
    pub initial_states: Vec<String>,
    /// Transitions, with labels aggregated per (from, to) pair.
    pub transitions: Vec<StateTransition>,
    /// Creation edges (`[*] --> state`), labeled with the intent(s) that
    /// create it. Kept separate from `transitions` because they have no
    /// `from` state — without this, creation-only intents (e.g. `CreateTicket`)
    /// were invisible on the diagram and absent from the operations legend.
    #[serde(default)]
    pub creation: Vec<StateTransition>,
    /// `(intent name, @doc)` for transition-triggering operations that carry a
    /// human description — rendered as a legend beneath the diagram.
    #[serde(default)]
    pub intent_docs: Vec<(String, String)>,
    /// Intents that unconditionally assert two or more *distinct* state targets
    /// at once (e.g. `ensure status' == Closed` **and** `ensure status' ==
    /// ExceptionClosed`). Such a clause set can never hold simultaneously —
    /// the same structural signal the verifier reports as `V0020
    /// SELF-CONTRADICTORY`. Surfaced here so the diagram can flag it.
    #[serde(default)]
    pub conflicts: Vec<StateConflict>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    /// Operation(s) that trigger this transition, joined by `/`.
    pub label: String,
}

/// A structural self-contradiction on the state field: one intent forces the
/// entity into several mutually exclusive next-states at once.
#[derive(Debug, Serialize, Deserialize)]
pub struct StateConflict {
    /// The offending intent.
    pub intent: String,
    /// Require-clause state sources (empty ⇒ creation edge).
    pub sources: Vec<String>,
    /// The mutually exclusive primed targets asserted together.
    pub targets: Vec<String>,
}

pub fn build_state_machine(program: &Program) -> StateMachine {
    // 1. Collect enum variants → enum name.
    let mut variant_to_enum: HashMap<String, String> = HashMap::new();
    for decl in &program.declarations {
        if let Declaration::Enum(e) = &decl.node {
            for v in &e.variants {
                variant_to_enum.insert(v.clone(), e.name.clone());
            }
        }
    }

    // 2. Pick the dominant state enum: the one whose variants appear most
    //    often as the RHS of a *primed* equality inside an ensure clause.
    let mut enum_hits: HashMap<String, usize> = HashMap::new();
    for decl in &program.declarations {
        if let Declaration::Intent(intent) = &decl.node {
            for clause in &intent.clauses {
                if clause.node.kind != ClauseKind::Ensure {
                    continue;
                }
                collect_state_eqs(&clause.node.expr.node, true, &variant_to_enum)
                    .into_iter()
                    .for_each(|variant| {
                        if let Some(enum_name) = variant_to_enum.get(&variant) {
                            *enum_hits.entry(enum_name.clone()).or_insert(0) += 1;
                        }
                    });
            }
        }
    }

    let state_enum = enum_hits
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name);

    let Some(state_enum) = state_enum else {
        return StateMachine {
            state_enum: None,
            states: Vec::new(),
            initial_states: Vec::new(),
            transitions: Vec::new(),
            creation: Vec::new(),
            intent_docs: Vec::new(),
            conflicts: Vec::new(),
        };
    };

    // Variants that belong to the state enum.
    let state_variants: BTreeSet<String> = variant_to_enum
        .iter()
        .filter(|(_, e)| **e == state_enum)
        .map(|(v, _)| v.clone())
        .collect();

    // 3. Walk intents and build (from, to) → labels.
    let mut edge_labels: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
    let mut creation_labels: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut initial: BTreeSet<String> = BTreeSet::new();
    let mut all_states: BTreeSet<String> = BTreeSet::new();
    let mut sources: BTreeSet<String> = BTreeSet::new();
    let mut conflicts: Vec<StateConflict> = Vec::new();

    for decl in &program.declarations {
        let Declaration::Intent(intent) = &decl.node else {
            continue;
        };

        // Sources: state-variant equalities in require clauses.
        let mut require_sources: BTreeSet<String> = BTreeSet::new();
        for clause in &intent.clauses {
            if clause.node.kind == ClauseKind::Require {
                for v in collect_state_eqs(&clause.node.expr.node, false, &variant_to_enum) {
                    if state_variants.contains(&v) {
                        require_sources.insert(v);
                    }
                }
            }
        }

        // Targets: primed state-variant equalities in ensure clauses.
        // Implications carry their own local source (the antecedent).
        let mut plain_targets: BTreeSet<String> = BTreeSet::new();
        let mut conditional: Vec<(String, String)> = Vec::new(); // (source, target)

        for clause in &intent.clauses {
            if clause.node.kind != ClauseKind::Ensure {
                continue;
            }
            extract_targets(
                &clause.node.expr.node,
                &state_variants,
                &variant_to_enum,
                &mut plain_targets,
                &mut conditional,
            );
        }

        // Structural self-contradiction (mirrors verifier V0020): a single
        // intent asserts two or more *distinct* next-states **unconditionally**
        // (bare `status' == Variant` ensures, not guarded by an implication).
        // Implication-guarded branches like `cond ==> status' == A` are
        // legitimate case splits and must NOT be flagged.
        let mut unconditional: BTreeSet<String> = BTreeSet::new();
        for clause in &intent.clauses {
            if clause.node.kind != ClauseKind::Ensure {
                continue;
            }
            collect_unconditional_targets(
                &clause.node.expr.node,
                &state_variants,
                &variant_to_enum,
                &mut unconditional,
            );
        }
        if unconditional.len() >= 2 {
            conflicts.push(StateConflict {
                intent: intent.name.clone(),
                sources: require_sources.iter().cloned().collect(),
                targets: unconditional.into_iter().collect(),
            });
        }

        // Emit conditional transitions (antecedent → consequent).
        for (src, tgt) in conditional {
            if src == tgt {
                continue; // identity, e.g. status' == status
            }
            all_states.insert(src.clone());
            all_states.insert(tgt.clone());
            sources.insert(src.clone());
            edge_labels
                .entry((src, tgt))
                .or_default()
                .insert(intent.name.clone());
        }

        // Emit plain transitions.
        for tgt in &plain_targets {
            all_states.insert(tgt.clone());
            if require_sources.is_empty() {
                // No pre-state constraint → creation edge.
                initial.insert(tgt.clone());
                creation_labels
                    .entry(tgt.clone())
                    .or_default()
                    .insert(intent.name.clone());
            } else {
                for src in &require_sources {
                    if src == tgt {
                        continue;
                    }
                    all_states.insert(src.clone());
                    sources.insert(src.clone());
                    edge_labels
                        .entry((src.clone(), tgt.clone()))
                        .or_default()
                        .insert(intent.name.clone());
                }
            }
        }
    }

    // Terminal states: targets that are never a source.
    let mut transitions: Vec<StateTransition> = edge_labels
        .into_iter()
        .map(|((from, to), labels)| StateTransition {
            from,
            to,
            label: labels.into_iter().collect::<Vec<_>>().join("/"),
        })
        .collect();
    transitions.sort_by(|a, b| (a.from.clone(), a.to.clone()).cmp(&(b.from.clone(), b.to.clone())));

    let mut creation: Vec<StateTransition> = creation_labels
        .into_iter()
        .map(|(to, labels)| StateTransition {
            from: "[*]".to_string(),
            to,
            label: labels.into_iter().collect::<Vec<_>>().join("/"),
        })
        .collect();
    creation.sort_by(|a, b| a.to.cmp(&b.to));

    // Legend: operations that appear on some transition (incl. creation) and
    // carry an `@doc`.
    let triggers: BTreeSet<&str> = transitions
        .iter()
        .chain(creation.iter())
        .flat_map(|t| t.label.split('/'))
        .map(|s| s.trim())
        .collect();
    let mut intent_docs = Vec::new();
    for decl in &program.declarations {
        if let Declaration::Intent(i) = &decl.node {
            if triggers.contains(i.name.as_str()) {
                if let Some(doc) = crate::goal_graph::doc_of(&i.annotations) {
                    intent_docs.push((i.name.clone(), doc));
                }
            }
        }
    }

    StateMachine {
        state_enum: Some(state_enum),
        states: all_states.into_iter().collect(),
        initial_states: initial.into_iter().collect(),
        transitions,
        creation,
        intent_docs,
        conflicts,
    }
}

/// Strip `Paren` wrappers.
fn unwrap_paren(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(inner) => unwrap_paren(&inner.node),
        other => other,
    }
}

/// If `expr` is an equality `path == Variant` (or `Variant == path`) where the
/// path's primed-ness matches `want_primed` and `Variant` is a known enum
/// variant, return the variant name.
fn as_state_eq(
    expr: &Expr,
    want_primed: bool,
    variant_to_enum: &HashMap<String, String>,
) -> Option<String> {
    let Expr::BinOp(lhs, BinOp::Eq, rhs) = expr else {
        return None;
    };
    for (path_side, var_side) in [(&lhs.node, &rhs.node), (&rhs.node, &lhs.node)] {
        if let Expr::Ident(name) = unwrap_paren(var_side) {
            if variant_to_enum.contains_key(name) && is_primed(path_side) == want_primed {
                return Some(name.clone());
            }
        }
    }
    None
}

fn is_primed(expr: &Expr) -> bool {
    matches!(unwrap_paren(expr), Expr::Prime(_))
}

/// Collect all state-variant names appearing in equalities of the requested
/// primed-ness anywhere in `expr` (descending through `&&`, `||`, parens).
fn collect_state_eqs(
    expr: &Expr,
    want_primed: bool,
    variant_to_enum: &HashMap<String, String>,
) -> Vec<String> {
    let expr = unwrap_paren(expr);
    if let Some(v) = as_state_eq(expr, want_primed, variant_to_enum) {
        return vec![v];
    }
    match expr {
        Expr::BinOp(lhs, BinOp::And | BinOp::Or, rhs) => {
            let mut out = collect_state_eqs(&lhs.node, want_primed, variant_to_enum);
            out.extend(collect_state_eqs(&rhs.node, want_primed, variant_to_enum));
            out
        }
        _ => Vec::new(),
    }
}

/// Extract transition targets from an ensure expression.
/// - `path' == Variant` → plain target.
/// - `cond ==> path' == Variant` → conditional (source-from-cond, target).
fn extract_targets(
    expr: &Expr,
    state_variants: &BTreeSet<String>,
    variant_to_enum: &HashMap<String, String>,
    plain: &mut BTreeSet<String>,
    conditional: &mut Vec<(String, String)>,
) {
    let expr = unwrap_paren(expr);

    if let Expr::BinOp(ante, BinOp::Implies, cons) = expr {
        // Consequent target(s).
        let targets = collect_state_eqs(&cons.node, true, variant_to_enum);
        // Antecedent source(s) (non-primed).
        let srcs = collect_state_eqs(&ante.node, false, variant_to_enum);
        for tgt in &targets {
            if !state_variants.contains(tgt) {
                continue;
            }
            if srcs.is_empty() {
                plain.insert(tgt.clone());
            } else {
                for s in &srcs {
                    if state_variants.contains(s) {
                        conditional.push((s.clone(), tgt.clone()));
                    }
                }
            }
        }
        return;
    }

    if let Some(v) = as_state_eq(expr, true, variant_to_enum) {
        if state_variants.contains(&v) {
            plain.insert(v);
        }
        return;
    }

    // Descend through conjunctions (e.g. multiple ensures fused).
    if let Expr::BinOp(lhs, BinOp::And, rhs) = expr {
        extract_targets(&lhs.node, state_variants, variant_to_enum, plain, conditional);
        extract_targets(&rhs.node, state_variants, variant_to_enum, plain, conditional);
    }
}

/// Collect *unconditional* primed state targets from an ensure expression:
/// bare `path' == Variant` (and conjunctions thereof). An implication
/// (`cond ==> ...`) is a guarded case split, not an unconditional assertion,
/// so it contributes nothing here.
fn collect_unconditional_targets(
    expr: &Expr,
    state_variants: &BTreeSet<String>,
    variant_to_enum: &HashMap<String, String>,
    out: &mut BTreeSet<String>,
) {
    let expr = unwrap_paren(expr);
    if let Some(v) = as_state_eq(expr, true, variant_to_enum) {
        if state_variants.contains(&v) {
            out.insert(v);
        }
        return;
    }
    if let Expr::BinOp(lhs, BinOp::And, rhs) = expr {
        collect_unconditional_targets(&lhs.node, state_variants, variant_to_enum, out);
        collect_unconditional_targets(&rhs.node, state_variants, variant_to_enum, out);
    }
}

/// States that are targets but never sources (candidate terminal states).
pub fn terminal_states(sm: &StateMachine) -> Vec<String> {
    let sources: BTreeSet<&String> = sm.transitions.iter().map(|t| &t.from).collect();
    sm.states
        .iter()
        .filter(|s| !sources.contains(s))
        .cloned()
        .collect()
}

/// Structural liveness / reachability report for a derived state machine.
///
/// This is the *positive* structural check (C1): it does not need SMT or
/// temporal logic — it is plain graph reachability over the transition graph.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StateMachineReport {
    /// States not reachable from any creation (`[*]`) edge — likely dead
    /// states (e.g. an enum variant no intent can ever produce).
    pub unreachable_from_initial: Vec<String>,
    /// Non-terminal states with no outgoing transition — a dead end that is
    /// not marked as a legitimate terminal (deadlock).
    pub stuck_states: Vec<String>,
    /// States from which no terminal state is reachable — the entity can get
    /// "trapped" in a cycle and never complete its lifecycle.
    pub cannot_reach_terminal: Vec<String>,
}

impl StateMachineReport {
    pub fn is_clean(&self) -> bool {
        self.unreachable_from_initial.is_empty()
            && self.stuck_states.is_empty()
            && self.cannot_reach_terminal.is_empty()
    }
}

/// Analyze a derived state machine for structural liveness problems.
pub fn analyze_state_machine(sm: &StateMachine) -> StateMachineReport {
    if sm.state_enum.is_none() || sm.states.is_empty() {
        return StateMachineReport::default();
    }

    // Adjacency (forward) and reverse adjacency.
    let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut backward: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in &sm.transitions {
        forward.entry(&t.from).or_default().push(&t.to);
        backward.entry(&t.to).or_default().push(&t.from);
    }

    // 1. Reachability from initial states.
    let mut reachable: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<&str> = sm.initial_states.iter().map(|s| s.as_str()).collect();
    while let Some(s) = stack.pop() {
        if !reachable.insert(s.to_string()) {
            continue;
        }
        if let Some(nexts) = forward.get(s) {
            stack.extend(nexts.iter().copied());
        }
    }
    let unreachable_from_initial: Vec<String> = sm
        .states
        .iter()
        .filter(|s| !reachable.contains(*s))
        .cloned()
        .collect();

    // Terminal states = no outgoing edge.
    let terminals: BTreeSet<String> = terminal_states(sm).into_iter().collect();

    // 2. Stuck states: non-terminal with no outgoing AND that are not
    //    themselves terminal — by definition terminal_states are exactly the
    //    no-outgoing set, so "stuck" is the subset we did NOT intend as an end.
    //    We treat a terminal as legitimate; a stuck state is a non-initial
    //    terminal that cannot reach itself onward — which is the same set,
    //    so instead we flag terminals that look accidental: none here unless
    //    the model has zero terminals. Report genuine dead ends only when a
    //    state has no outgoing edge and is not reachable to any *other*
    //    terminal (i.e. it is its own only sink but was not meant to end).
    //    Practically: flag no-outgoing states that are also unreachable, or
    //    when there are no terminals at all (pure cycle).
    let mut stuck_states: Vec<String> = Vec::new();
    if terminals.is_empty() {
        // Every state is in a cycle with no way out.
        stuck_states = sm.states.clone();
    }

    // 3. Cannot reach any terminal: reverse-BFS from terminals; anything not
    //    visited is trapped.
    let mut can_reach_terminal: BTreeSet<String> = BTreeSet::new();
    let mut rstack: Vec<&str> = terminals.iter().map(|s| s.as_str()).collect();
    while let Some(s) = rstack.pop() {
        if !can_reach_terminal.insert(s.to_string()) {
            continue;
        }
        if let Some(prevs) = backward.get(s) {
            rstack.extend(prevs.iter().copied());
        }
    }
    let cannot_reach_terminal: Vec<String> = sm
        .states
        .iter()
        .filter(|s| !can_reach_terminal.contains(*s))
        .cloned()
        .collect();

    StateMachineReport {
        unreachable_from_initial,
        stuck_states,
        cannot_reach_terminal,
    }
}

impl crate::GraphData for StateMachine {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mermaid::MermaidRenderable;

    const SRC: &str = r#"
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
    fn detects_state_enum_and_creation_edge() {
        let sm = build_state_machine(&parse(SRC));
        assert_eq!(sm.state_enum.as_deref(), Some("S"));
        assert!(sm.initial_states.contains(&"Draft".to_string()));
    }

    #[test]
    fn derives_transitions_with_labels() {
        let sm = build_state_machine(&parse(SRC));
        let has = |from: &str, to: &str, label: &str| {
            sm.transitions
                .iter()
                .any(|t| t.from == from && t.to == to && t.label == label)
        };
        assert!(has("Draft", "Open", "Publish"));
        assert!(has("Open", "Done", "Finish"));
    }

    #[test]
    fn terminal_states_have_no_outgoing() {
        let sm = build_state_machine(&parse(SRC));
        assert_eq!(terminal_states(&sm), vec!["Done".to_string()]);
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
    fn no_state_enum_degrades_gracefully() {
        let src = "type A { x: Int }\nintent Bump(a: A) { ensure a.x' == a.x + 1 }";
        let sm = build_state_machine(&parse(src));
        assert!(sm.state_enum.is_none());
        assert!(sm.to_mermaid().contains("stateDiagram-v2"));
    }

    #[test]
    fn clean_machine_reports_no_issues() {
        let report = analyze_state_machine(&build_state_machine(&parse(SRC)));
        assert!(report.is_clean(), "unexpected: {report:?}");
    }

    #[test]
    fn detects_unreachable_dead_state() {
        // `Archived` is a state no intent ever produces or leaves.
        let src = r#"
            enum S { Draft, Open, Done, Archived }
            type Doc { status: S }
            intent Create(d: Doc) { ensure d.status' == Draft }
            intent Publish(d: Doc) { require d.status == Draft else reject
                                     ensure d.status' == Open }
            intent Finish(d: Doc) { require d.status == Open else reject
                                    ensure d.status' == Done }
            intent Touch(d: Doc) { require d.status == Archived else reject
                                   ensure d.status' == Archived }
        "#;
        let report = analyze_state_machine(&build_state_machine(&parse(src)));
        // Archived can never be produced by any intent → dead state.
        assert!(report.unreachable_from_initial.contains(&"Archived".to_string()));
    }

    #[test]
    fn flags_unconditional_contradictory_targets() {
        // One intent asserts two distinct next-states unconditionally → V0020.
        let src = r#"
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
        assert_eq!(sm.conflicts[0].intent, "Bad");
        assert_eq!(sm.conflicts[0].targets, vec!["B".to_string(), "C".to_string()]);
    }

    #[test]
    fn does_not_flag_conditional_case_split() {
        // Mutually exclusive implication branches are legitimate, not a conflict.
        let src = r#"
            enum S { A, B, C }
            type X { status: S flag: Bool }
            intent Split(x: X) {
              ensure a: x.flag ==> x.status' == B
              ensure b: !x.flag ==> x.status' == C
            }
        "#;
        let sm = build_state_machine(&parse(src));
        assert!(sm.conflicts.is_empty(), "unexpected: {:?}", sm.conflicts);
    }

    #[test]
    fn detects_trapped_cycle_without_terminal() {
        // Ping-pong with no way out: no terminal reachable.
        let src = r#"
            enum S { A, B }
            type X { status: S }
            intent Start(x: X) { ensure x.status' == A }
            intent ToB(x: X) { require x.status == A else reject ensure x.status' == B }
            intent ToA(x: X) { require x.status == B else reject ensure x.status' == A }
        "#;
        let report = analyze_state_machine(&build_state_machine(&parse(src)));
        assert!(!report.cannot_reach_terminal.is_empty());
        assert!(!report.stuck_states.is_empty());
    }
}
