//! Structural queries over a parsed program — no SMT, no rendering.
//!
//! Everything here is derived from the AST alone:
//! - **state machines**: `require path == Variant` (source) paired with
//!   `ensure path' == Variant` (target), scoped to one enum;
//! - **goal coverage**: which intents / safeties no `goal` claims via
//!   `realized_by`;
//! - **example coverage**: which intents carry no `example` block.
//!
//! This module lives in the syntax crate so that both the verifier
//! (`intent check`, which owns the *policy* — see `intent_lang_core::structure`)
//! and the visualizer (which owns the *rendering*) derive from one
//! implementation. Keeping it here also keeps the visualizer free of the
//! verifier's Z3 dependency.
//!
//! # Scoping: `@lifecycle`
//!
//! State-machine analysis runs against **declared** lifecycles
//! (`@lifecycle enum Status { ... }`), not guessed ones. A file with no
//! `@lifecycle` has no state machine — which is the correct answer for
//! domains like transfer or sorting that genuinely have no lifecycle, and
//! avoids reporting made-up defects against them. `dominant_state_enum`
//! remains available as a legacy heuristic for callers that must guess.

use crate::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

// ── State machine ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct StateMachine {
    /// The enum used as the state space (if any).
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
    /// SELF-CONTRADICTORY`.
    #[serde(default)]
    pub conflicts: Vec<StateConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    /// Operation(s) that trigger this transition, joined by `/`.
    pub label: String,
}

/// A structural self-contradiction on the state field: one intent forces the
/// entity into several mutually exclusive next-states at once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConflict {
    /// The offending intent.
    pub intent: String,
    /// Require-clause state sources (empty ⇒ creation edge).
    pub sources: Vec<String>,
    /// The mutually exclusive primed targets asserted together.
    pub targets: Vec<String>,
}

impl StateMachine {
    fn empty() -> Self {
        StateMachine {
            state_enum: None,
            states: Vec::new(),
            initial_states: Vec::new(),
            transitions: Vec::new(),
            creation: Vec::new(),
            intent_docs: Vec::new(),
            conflicts: Vec::new(),
        }
    }
}

/// Enums carrying `@lifecycle`, in declaration order.
pub fn lifecycle_enums(program: &Program) -> Vec<String> {
    program
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Enum(e) if e.is_lifecycle() => Some(e.name.clone()),
            _ => None,
        })
        .collect()
}

/// Legacy heuristic: the enum whose variants appear most often as the RHS of a
/// primed equality. Prefer [`lifecycle_enums`] — guessing cannot tell a
/// lifecycle apart from an ordinary enum like `Priority`, and analyzing the
/// latter as a state machine produces made-up "unreachable state" findings.
pub fn dominant_state_enum(program: &Program) -> Option<String> {
    let variant_to_enum = variant_index(program);
    let mut hits: HashMap<String, usize> = HashMap::new();
    for decl in &program.declarations {
        if let Declaration::Intent(intent) = &decl.node {
            for clause in &intent.clauses {
                if clause.node.kind != ClauseKind::Ensure {
                    continue;
                }
                for variant in collect_state_eqs(&clause.node.expr.node, true, &variant_to_enum) {
                    if let Some(enum_name) = variant_to_enum.get(&variant) {
                        *hits.entry(enum_name.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    hits.into_iter().max_by_key(|(_, c)| *c).map(|(n, _)| n)
}

/// One state machine per declared `@lifecycle` enum, in declaration order.
pub fn lifecycle_state_machines(program: &Program) -> Vec<StateMachine> {
    lifecycle_enums(program)
        .into_iter()
        .map(|e| build_state_machine_for(program, &e))
        .collect()
}

/// The state machine a single-diagram consumer should show: the first declared
/// `@lifecycle`, falling back to the dominant-enum heuristic.
pub fn build_state_machine(program: &Program) -> StateMachine {
    let chosen = lifecycle_enums(program)
        .into_iter()
        .next()
        .or_else(|| dominant_state_enum(program));
    match chosen {
        Some(e) => build_state_machine_for(program, &e),
        None => StateMachine::empty(),
    }
}

fn variant_index(program: &Program) -> HashMap<String, String> {
    let mut variant_to_enum = HashMap::new();
    for decl in &program.declarations {
        if let Declaration::Enum(e) = &decl.node {
            for v in &e.variants {
                variant_to_enum.insert(v.clone(), e.name.clone());
            }
        }
    }
    variant_to_enum
}

/// Derive the state machine over `state_enum`'s variants.
pub fn build_state_machine_for(program: &Program, state_enum: &str) -> StateMachine {
    let variant_to_enum = variant_index(program);
    let state_variants: BTreeSet<String> = variant_to_enum
        .iter()
        .filter(|(_, e)| e.as_str() == state_enum)
        .map(|(v, _)| v.clone())
        .collect();

    if state_variants.is_empty() {
        return StateMachine::empty();
    }

    let mut edge_labels: HashMap<(String, String), BTreeSet<String>> = HashMap::new();
    let mut creation_labels: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut initial: BTreeSet<String> = BTreeSet::new();
    let mut all_states: BTreeSet<String> = BTreeSet::new();
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
        let mut conditional: Vec<(String, String)> = Vec::new();
        let mut unconditional: BTreeSet<String> = BTreeSet::new();

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
            collect_unconditional_targets(
                &clause.node.expr.node,
                &state_variants,
                &variant_to_enum,
                &mut unconditional,
            );
        }

        // Structural self-contradiction (mirrors verifier V0020): a single
        // intent asserts two or more *distinct* next-states **unconditionally**.
        // Implication-guarded branches are legitimate case splits.
        if unconditional.len() >= 2 {
            conflicts.push(StateConflict {
                intent: intent.name.clone(),
                sources: require_sources.iter().cloned().collect(),
                targets: unconditional.into_iter().collect(),
            });
        }

        for (src, tgt) in conditional {
            if src == tgt {
                continue; // identity, e.g. status' == status
            }
            all_states.insert(src.clone());
            all_states.insert(tgt.clone());
            edge_labels
                .entry((src, tgt))
                .or_default()
                .insert(intent.name.clone());
        }

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
                    edge_labels
                        .entry((src.clone(), tgt.clone()))
                        .or_default()
                        .insert(intent.name.clone());
                }
            }
        }
    }

    let mut transitions: Vec<StateTransition> = edge_labels
        .into_iter()
        .map(|((from, to), labels)| StateTransition {
            from,
            to,
            label: labels.into_iter().collect::<Vec<_>>().join("/"),
        })
        .collect();
    transitions.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));

    let mut creation: Vec<StateTransition> = creation_labels
        .into_iter()
        .map(|(to, labels)| StateTransition {
            from: "[*]".to_string(),
            to,
            label: labels.into_iter().collect::<Vec<_>>().join("/"),
        })
        .collect();
    creation.sort_by(|a, b| a.to.cmp(&b.to));

    // Legend: operations on some transition (incl. creation) carrying an `@doc`.
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
                if let Some(doc) = doc_of(&i.annotations) {
                    intent_docs.push((i.name.clone(), doc));
                }
            }
        }
    }

    StateMachine {
        state_enum: Some(state_enum.to_string()),
        states: all_states.into_iter().collect(),
        initial_states: initial.into_iter().collect(),
        transitions,
        creation,
        intent_docs,
        conflicts,
    }
}

/// Extract the `@doc("...")` one-line description from a set of annotations.
pub fn doc_of(annotations: &[Annotation]) -> Option<String> {
    annotations.iter().find(|a| a.name == "doc").and_then(|a| {
        a.args.iter().find_map(|arg| match arg {
            AnnotationArg::Positional(e) => match &e.node {
                Expr::StringLit(s) => Some(s.clone()),
                _ => None,
            },
            AnnotationArg::Named(_, _) => None,
        })
    })
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
        let targets = collect_state_eqs(&cons.node, true, variant_to_enum);
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

    if let Expr::BinOp(lhs, BinOp::And, rhs) = expr {
        extract_targets(
            &lhs.node,
            state_variants,
            variant_to_enum,
            plain,
            conditional,
        );
        extract_targets(
            &rhs.node,
            state_variants,
            variant_to_enum,
            plain,
            conditional,
        );
    }
}

/// Collect *unconditional* primed state targets: bare `path' == Variant` (and
/// conjunctions thereof). An implication is a guarded case split, not an
/// unconditional assertion, so it contributes nothing here.
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
/// Plain graph reachability over the transition graph — no SMT, no temporal
/// logic. Interpreting the findings (which are defects, which are legitimate
/// modeling choices) is the caller's job; see `intent_lang_core::structure`.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StateMachineReport {
    /// States not reachable from any creation (`[*]`) edge. Only meaningful
    /// when the machine *has* creation edges — see `has_creation`.
    pub unreachable_from_initial: Vec<String>,
    /// True when at least one intent creates an entity without requiring a
    /// pre-state. Without it, "unreachable" says nothing: the file may simply
    /// model the middle of a lifecycle whose creation lives elsewhere.
    pub has_creation: bool,
    /// True when at least one state has no outgoing transition.
    pub has_terminal: bool,
    /// States from which no terminal state is reachable — the entity can get
    /// "trapped" in a cycle and never complete its lifecycle.
    pub cannot_reach_terminal: Vec<String>,
    /// Every state entered by a creation edge, sorted. More than one entry point
    /// is the symptom of a lifecycle whose chain has been cut: an intent that
    /// asserts a next-state while sourcing its precondition from a boolean flag
    /// (`require ctx.devicesFound` instead of `require ctx.phase ==
    /// DevicesResolved`) has no state source, so it is indistinguishable from a
    /// bootstrap. It also makes `unreachable_from_initial` vacuous — a state
    /// with its own creation edge is trivially reachable.
    #[serde(default)]
    pub creation_targets: Vec<String>,
}

impl StateMachineReport {
    pub fn is_clean(&self) -> bool {
        self.unreachable_from_initial.is_empty()
            && self.has_creation
            && self.creation_targets.len() <= 1
            && self.has_terminal
            && self.cannot_reach_terminal.is_empty()
    }
}

/// Analyze a derived state machine for structural liveness problems.
pub fn analyze_state_machine(sm: &StateMachine) -> StateMachineReport {
    if sm.state_enum.is_none() || sm.states.is_empty() {
        return StateMachineReport {
            has_creation: true,
            has_terminal: true,
            ..Default::default()
        };
    }

    let mut forward: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut backward: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in &sm.transitions {
        forward.entry(&t.from).or_default().push(&t.to);
        backward.entry(&t.to).or_default().push(&t.from);
    }

    let has_creation = !sm.initial_states.is_empty();

    // Reachability from creation edges. Without any creation edge every state
    // would be "unreachable", which is noise rather than signal — the missing
    // creation edge is the one thing worth reporting.
    let unreachable_from_initial = if has_creation {
        let mut reachable: BTreeSet<&str> = BTreeSet::new();
        let mut stack: Vec<&str> = sm.initial_states.iter().map(|s| s.as_str()).collect();
        while let Some(s) = stack.pop() {
            if !reachable.insert(s) {
                continue;
            }
            if let Some(nexts) = forward.get(s) {
                stack.extend(nexts.iter().copied());
            }
        }
        sm.states
            .iter()
            .filter(|s| !reachable.contains(s.as_str()))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    let terminals: BTreeSet<String> = terminal_states(sm).into_iter().collect();
    let has_terminal = !terminals.is_empty();

    // Reverse-BFS from terminals; anything not visited is trapped.
    let cannot_reach_terminal = if has_terminal {
        let mut can_reach: BTreeSet<&str> = BTreeSet::new();
        let mut stack: Vec<&str> = terminals.iter().map(|s| s.as_str()).collect();
        while let Some(s) = stack.pop() {
            if !can_reach.insert(s) {
                continue;
            }
            if let Some(prevs) = backward.get(s) {
                stack.extend(prevs.iter().copied());
            }
        }
        sm.states
            .iter()
            .filter(|s| !can_reach.contains(s.as_str()))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    StateMachineReport {
        unreachable_from_initial,
        has_creation,
        has_terminal,
        cannot_reach_terminal,
        creation_targets: sm.initial_states.clone(),
    }
}

// ── Goal / example coverage ──────────────────────────────────

/// What kind of declaration a `realized_by` entry points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RealizableKind {
    Intent,
    Safety,
}

/// An intent / safety that no `goal` claims through `realized_by`.
#[derive(Debug, Clone)]
pub struct Unclaimed {
    pub name: String,
    pub kind: RealizableKind,
    pub span: Span,
}

/// Intents and safeties not referenced by any goal's `realized_by`, in
/// declaration order. A coverage gap: the file states rules nobody asked for,
/// or a goal forgot to list its realizer.
pub fn unclaimed_realizables(program: &Program) -> Vec<Unclaimed> {
    let mut claimed: BTreeSet<&str> = BTreeSet::new();
    for decl in &program.declarations {
        if let Declaration::Goal(g) = &decl.node {
            claimed.extend(g.realized_by.iter().map(|s| s.as_str()));
        }
    }

    let mut out = Vec::new();
    for decl in &program.declarations {
        let (name, kind) = match &decl.node {
            Declaration::Intent(i) => (&i.name, RealizableKind::Intent),
            Declaration::Safety(s) => (&s.name, RealizableKind::Safety),
            _ => continue,
        };
        if !claimed.contains(name.as_str()) {
            out.push(Unclaimed {
                name: name.clone(),
                kind,
                span: decl.span.clone(),
            });
        }
    }
    out
}

/// Intents with no `example` block, in declaration order. Examples are the
/// defense against formalization drift (Z3 checks them against the clauses)
/// and the first batch of acceptance data.
pub fn intents_without_example(program: &Program) -> Vec<(String, Span)> {
    let mut exemplified: BTreeSet<&str> = BTreeSet::new();
    for decl in &program.declarations {
        if let Declaration::Example(e) = &decl.node {
            exemplified.insert(e.intent.as_str());
        }
    }

    program
        .declarations
        .iter()
        .filter_map(|decl| match &decl.node {
            Declaration::Intent(i) if !exemplified.contains(i.name.as_str()) => {
                Some((i.name.clone(), decl.span.clone()))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        crate::parse(src).expect("parse")
    }

    const LIFECYCLE: &str = r#"
        @lifecycle
        enum S { Draft, Open, Done }
        type Doc { status: S }
        intent Create(d: Doc) { ensure d.status' == Draft }
        intent Publish(d: Doc) {
          require d.status == Draft else reject
          ensure d.status' == Open
        }
        intent Finish(d: Doc) {
          require d.status == Open else reject
          ensure d.status' == Done
        }
    "#;

    #[test]
    fn lifecycle_annotation_selects_the_state_enum() {
        let sm = build_state_machine(&parse(LIFECYCLE));
        assert_eq!(sm.state_enum.as_deref(), Some("S"));
        assert!(sm.initial_states.contains(&"Draft".to_string()));
        assert_eq!(sm.transitions.len(), 2);
    }

    #[test]
    fn clean_lifecycle_reports_no_issues() {
        let report = analyze_state_machine(&build_state_machine(&parse(LIFECYCLE)));
        assert!(report.is_clean(), "{report:?}");
    }

    #[test]
    fn no_lifecycle_annotation_yields_no_machine_to_analyze() {
        // Priority is an ordinary enum, not a lifecycle: analyzing it as one
        // would invent "unreachable state" findings.
        let src = r#"
            enum Priority { Low, High }
            type T { p: Priority }
            intent Raise(t: T) { ensure t.p' == High }
        "#;
        let program = parse(src);
        assert!(lifecycle_enums(&program).is_empty());
        assert!(lifecycle_state_machines(&program).is_empty());
    }

    #[test]
    fn missing_creation_edge_suppresses_unreachable_noise() {
        let src = r#"
            @lifecycle
            enum S { Open, Done }
            type Doc { status: S }
            intent Finish(d: Doc) {
              require d.status == Open else reject
              ensure d.status' == Done
            }
        "#;
        let report = analyze_state_machine(&build_state_machine(&parse(src)));
        assert!(!report.has_creation);
        assert!(
            report.unreachable_from_initial.is_empty(),
            "every state is trivially unreachable without a creation edge; \
             reporting them all is noise"
        );
    }

    #[test]
    fn unreachable_state_detected_when_creation_exists() {
        let src = r#"
            @lifecycle
            enum S { Draft, Orphan, Done }
            type Doc { status: S }
            intent Create(d: Doc) { ensure d.status' == Draft }
            intent Finish(d: Doc) {
              require d.status == Draft else reject
              ensure d.status' == Done
            }
            intent Leave(d: Doc) {
              require d.status == Orphan else reject
              ensure d.status' == Done
            }
        "#;
        let report = analyze_state_machine(&build_state_machine(&parse(src)));
        assert!(report.has_creation);
        assert_eq!(report.unreachable_from_initial, vec!["Orphan".to_string()]);
    }

    #[test]
    fn multiple_lifecycles_each_get_a_machine() {
        let src = r#"
            @lifecycle
            enum RegistrationPhase { Entry, Registered }
            @lifecycle
            enum SnQueryPhase { Idle, Queried }
            type D { phase: RegistrationPhase  sn: SnQueryPhase }
            intent Register(d: D) { ensure d.phase' == Registered }
            intent Query(d: D) { ensure d.sn' == Queried }
        "#;
        let machines = lifecycle_state_machines(&parse(src));
        assert_eq!(machines.len(), 2);
        assert_eq!(machines[0].state_enum.as_deref(), Some("RegistrationPhase"));
        assert_eq!(machines[1].state_enum.as_deref(), Some("SnQueryPhase"));
    }

    #[test]
    fn self_contradictory_transition_detected() {
        let src = r#"
            @lifecycle
            enum S { Open, Closed, Exception }
            type Doc { status: S }
            intent Close(d: Doc) {
              require d.status == Open else reject
              ensure a: d.status' == Closed
              ensure b: d.status' == Exception
            }
        "#;
        let sm = build_state_machine(&parse(src));
        assert_eq!(sm.conflicts.len(), 1);
        assert_eq!(sm.conflicts[0].intent, "Close");
    }

    #[test]
    fn unclaimed_realizables_found() {
        let src = r#"
            type A { x: Int }
            goal "g" { realized_by: [Op] }
            intent Op(a: A) { ensure a.x' == a.x + 1 }
            intent Lonely(a: A) { ensure a.x' == a.x + 1 }
        "#;
        let unclaimed = unclaimed_realizables(&parse(src));
        assert_eq!(unclaimed.len(), 1);
        assert_eq!(unclaimed[0].name, "Lonely");
        assert_eq!(unclaimed[0].kind, RealizableKind::Intent);
    }

    #[test]
    fn intents_without_example_found() {
        let src = r#"
            type A { x: Int }
            intent Op(a: A) { ensure a.x' == a.x + 1 }
            intent Bare(a: A) { ensure a.x' == a.x + 2 }
            example Op "bump" { given: { a.x: 1 } expect: { a.x': 2 } }
        "#;
        let bare = intents_without_example(&parse(src));
        assert_eq!(bare.len(), 1);
        assert_eq!(bare[0].0, "Bare");
    }
}
