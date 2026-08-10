use intent_lang_syntax::ast::*;
use std::collections::{BTreeSet, HashMap, HashSet};

/// A verification condition to be checked by SMT.
#[derive(Debug, Clone)]
pub struct VerificationCondition {
    pub name: String,
    pub kind: VcKind,
    /// Declarations needed (types, uninterpreted functions).
    pub declarations: Vec<SmtDecl>,
    /// Assertions: requires + invariants(unprimed).
    pub assumes: Vec<Spanned<Expr>>,
    /// Goal: ensures + invariants(as written, may contain primes).
    pub goals: Vec<Spanned<Expr>>,
    /// Safety rules merged from `safety` declarations.
    pub safety_rules: Vec<SafetySource>,
    /// If set, this VC cannot be encoded yet — reason string.
    pub unsupported: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SafetySource {
    pub safety_name: String,
    pub index: usize,
    pub expr: Spanned<Expr>,
    /// The `safety` block's parameters. A rule's variables are free symbols
    /// bound by name, so a rule only governs intents that declare the same
    /// parameters — see [`safety_applies_to`].
    pub params: Vec<Param>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcKind {
    Intent,
    Theorem,
}

#[derive(Debug, Clone)]
pub enum SmtDecl {
    DeclareSort(String),
    DeclareConst(String, TypeExpr),
    DeclareFun(String, Vec<TypeExpr>, TypeExpr),
}

/// D2 frame semantics: collect the frame of an intent — the set of state
/// paths (flattened, e.g. `sender.balance`) the intent may modify.
///
/// - explicit `modifies a.b, c.d` → exactly those paths;
/// - no `modifies` → inferred as every path that appears primed in
///   **ensure** clauses. Invariants are deliberately excluded: an ensure
///   is an *effect declaration* ("this changes"), an invariant is a
///   *proof obligation* ("prove this still holds") — counting invariant
///   primes into the frame would make `invariant x' == x` unprovable
///   by its own mention;
/// - `modifies *` → `None` (frame semantics opted out).
///
/// Returns `None` for wildcard, otherwise the set of flattened paths.
pub fn intent_frame(intent: &IntentDecl) -> Option<BTreeSet<String>> {
    match &intent.modifies {
        Some(ModifiesSpec::Wildcard) => None,
        Some(ModifiesSpec::Paths(paths)) => {
            Some(paths.iter().filter_map(|p| expr_path(&p.node)).collect())
        }
        None => {
            let mut frame = BTreeSet::new();
            for cl in &intent.clauses {
                if cl.node.kind == ClauseKind::Ensure {
                    collect_primed_paths(&cl.node.expr, &mut frame);
                }
            }
            Some(frame)
        }
    }
}

/// Flatten a path expression (`sender.balance`) into `"sender.balance"`.
/// Returns None for non-path expressions.
fn expr_path(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(n) => Some(n.clone()),
        Expr::FieldAccess(base, field) => Some(format!("{}.{field}", expr_path(&base.node)?)),
        Expr::Prime(inner) | Expr::Paren(inner) => expr_path(&inner.node),
        _ => None,
    }
}

/// Collect all paths that appear under a Prime node.
fn collect_primed_paths(expr: &Spanned<Expr>, out: &mut BTreeSet<String>) {
    match &expr.node {
        Expr::Prime(inner) => {
            if let Some(p) = expr_path(&inner.node) {
                out.insert(p);
            }
            collect_primed_paths(inner, out);
        }
        Expr::FieldAccess(base, _) => collect_primed_paths(base, out),
        Expr::Index(b, i) => {
            collect_primed_paths(b, out);
            collect_primed_paths(i, out);
        }
        Expr::BinOp(l, _, r) => {
            collect_primed_paths(l, out);
            collect_primed_paths(r, out);
        }
        Expr::UnaryOp(_, o) => collect_primed_paths(o, out),
        Expr::IfThenElse(c, t, e) => {
            collect_primed_paths(c, out);
            collect_primed_paths(t, out);
            collect_primed_paths(e, out);
        }
        Expr::Forall(_, b) | Expr::Exists(_, b) => collect_primed_paths(b, out),
        Expr::Call(_, args) => {
            for a in args {
                collect_primed_paths(a, out);
            }
        }
        Expr::Paren(inner) => collect_primed_paths(inner, out),
        _ => {}
    }
}

/// Build a path expression AST node from a flattened path like `sender.balance`.
fn path_to_expr(path: &str) -> Spanned<Expr> {
    let span = Span::new(0, 0);
    let mut parts = path.split('.');
    let first = parts.next().unwrap();
    let mut e = Spanned::new(Expr::Ident(first.to_string()), span.clone());
    for p in parts {
        e = Spanned::new(Expr::FieldAccess(Box::new(e), p.to_string()), span.clone());
    }
    e
}

/// Convenience: frame equalities for an intent, deriving the struct-field
/// map from the program. Used by example checking (D5) and acceptance.
pub fn frame_equalities_for(prog: &Program, intent: &IntentDecl) -> Vec<Spanned<Expr>> {
    let struct_fields: HashMap<String, Vec<String>> = prog
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Type(t) => Some((
                t.name.clone(),
                t.fields.iter().map(|f| f.name.clone()).collect(),
            )),
            _ => None,
        })
        .collect();
    match intent_frame(intent) {
        Some(frame) => frame_equalities(intent, &struct_fields, &frame),
        None => Vec::new(),
    }
}

/// D2: generate frame equalities `x' == x` for every observable scalar
/// field of the intent's struct params (and scalar params) that is NOT
/// in the frame. Returns the equalities to be assumed.
fn frame_equalities(
    intent: &IntentDecl,
    struct_fields: &HashMap<String, Vec<String>>,
    frame: &BTreeSet<String>,
) -> Vec<Spanned<Expr>> {
    let span = Span::new(0, 0);
    let mut eqs = Vec::new();
    for p in &intent.params {
        let ty_name = match &p.ty {
            TypeExpr::Named(n) => n.clone(),
            TypeExpr::Qualified(m, n) => format!("{m}.{n}"),
            TypeExpr::Generic(..) => continue, // Seq/Set: no frame yet
        };
        if let Some(fields) = struct_fields.get(&ty_name) {
            for f in fields {
                let path = format!("{}.{}", p.name, f);
                if !frame.contains(&path) {
                    let base = path_to_expr(&path);
                    let primed = Spanned::new(Expr::Prime(Box::new(base.clone())), span.clone());
                    eqs.push(Spanned::new(
                        Expr::BinOp(Box::new(primed), BinOp::Eq, Box::new(base)),
                        span.clone(),
                    ));
                }
            }
        }
        // Scalar params (Int/Bool/String) are inputs, not state — no frame.
    }
    eqs
}

/// Remove all Prime nodes from an expression (replace `x'` with `x`).
pub fn unprime_expr(expr: &Spanned<Expr>) -> Spanned<Expr> {
    let node = match &expr.node {
        Expr::Prime(inner) => return unprime_expr(inner),
        Expr::IntLit(_) | Expr::BoolLit(_) | Expr::StringLit(_) | Expr::Ident(_) => {
            return expr.clone()
        }
        Expr::FieldAccess(base, field) => {
            Expr::FieldAccess(Box::new(unprime_expr(base)), field.clone())
        }
        Expr::Index(base, idx) => {
            Expr::Index(Box::new(unprime_expr(base)), Box::new(unprime_expr(idx)))
        }
        Expr::BinOp(l, op, r) => {
            Expr::BinOp(Box::new(unprime_expr(l)), *op, Box::new(unprime_expr(r)))
        }
        Expr::UnaryOp(op, o) => Expr::UnaryOp(*op, Box::new(unprime_expr(o))),
        Expr::IfThenElse(c, t, e) => Expr::IfThenElse(
            Box::new(unprime_expr(c)),
            Box::new(unprime_expr(t)),
            Box::new(unprime_expr(e)),
        ),
        Expr::Forall(vars, body) => Expr::Forall(vars.clone(), Box::new(unprime_expr(body))),
        Expr::Exists(vars, body) => Expr::Exists(vars.clone(), Box::new(unprime_expr(body))),
        Expr::Call(name, args) => Expr::Call(name.clone(), args.iter().map(unprime_expr).collect()),
        Expr::Paren(inner) => Expr::Paren(Box::new(unprime_expr(inner))),
    };
    Spanned::new(node, expr.span.clone())
}

/// Does this expression mention the post-state anywhere?
fn mentions_post_state(expr: &Spanned<Expr>) -> bool {
    match &expr.node {
        Expr::Prime(_) => true,
        Expr::IntLit(_) | Expr::BoolLit(_) | Expr::StringLit(_) | Expr::Ident(_) => false,
        Expr::FieldAccess(base, _) => mentions_post_state(base),
        Expr::Index(b, i) => mentions_post_state(b) || mentions_post_state(i),
        Expr::BinOp(l, _, r) => mentions_post_state(l) || mentions_post_state(r),
        Expr::UnaryOp(_, o) => mentions_post_state(o),
        Expr::IfThenElse(c, t, e) => {
            mentions_post_state(c) || mentions_post_state(t) || mentions_post_state(e)
        }
        Expr::Forall(_, body) | Expr::Exists(_, body) => mentions_post_state(body),
        Expr::Call(_, args) => args.iter().any(mentions_post_state),
        Expr::Paren(inner) => mentions_post_state(inner),
    }
}

/// Rewrite every state reference to its post-state counterpart.
///
/// Only field accesses are state: a bare identifier is a scalar parameter
/// (`amount`) or an enum variant (`Active`), and priming one of those would
/// invent a symbol nothing constrains, turning a provable goal into a
/// spurious failure.
fn prime_state(expr: &Spanned<Expr>) -> Spanned<Expr> {
    let node = match &expr.node {
        Expr::Prime(_) => return expr.clone(),
        Expr::FieldAccess(..) => {
            return Spanned::new(Expr::Prime(Box::new(expr.clone())), expr.span.clone())
        }
        Expr::IntLit(_) | Expr::BoolLit(_) | Expr::StringLit(_) | Expr::Ident(_) => {
            return expr.clone()
        }
        Expr::Index(base, idx) => {
            Expr::Index(Box::new(prime_state(base)), Box::new(prime_state(idx)))
        }
        Expr::BinOp(l, op, r) => {
            Expr::BinOp(Box::new(prime_state(l)), *op, Box::new(prime_state(r)))
        }
        Expr::UnaryOp(op, o) => Expr::UnaryOp(*op, Box::new(prime_state(o))),
        Expr::IfThenElse(c, t, e) => Expr::IfThenElse(
            Box::new(prime_state(c)),
            Box::new(prime_state(t)),
            Box::new(prime_state(e)),
        ),
        Expr::Forall(vars, body) => Expr::Forall(vars.clone(), Box::new(prime_state(body))),
        Expr::Exists(vars, body) => Expr::Exists(vars.clone(), Box::new(prime_state(body))),
        Expr::Call(name, args) => Expr::Call(name.clone(), args.iter().map(prime_state).collect()),
        Expr::Paren(inner) => Expr::Paren(Box::new(prime_state(inner))),
    };
    Spanned::new(node, expr.span.clone())
}

/// The proof obligation an `invariant` (on an intent or in a `safety` block)
/// places on an operation.
///
/// "Invariant" means the property holds in every state, so the operation must
/// leave it holding — the obligation is about the **post**-state. Taking the
/// expression as written only works when the author primed it by hand. When
/// they did not, the same formula ends up as both an assumption and the
/// negated goal (`(assert (>= b 0))` next to `(assert (not (>= b 0)))`),
/// which is unsatisfiable no matter what the operation does, so the check
/// passed unconditionally and proved nothing. Every unprimed `safety` in this
/// repository was vacuous that way, including the nine in
/// `examples/requirements/ticket.intent`.
///
/// An expression that mentions the post-state anywhere is left alone: it is
/// deliberately relating the two states (`a.balance' >= a.balance`), and
/// priming the rest would collapse it into a tautology — the very failure
/// being fixed.
pub fn invariant_goal(expr: &Spanned<Expr>) -> Spanned<Expr> {
    if mentions_post_state(expr) {
        expr.clone()
    } else {
        prime_state(expr)
    }
}

/// Does this safety rule say anything about the state `intent` operates on?
///
/// A rule's variables are free symbols matched by name: `safety Cap(c:
/// Customer)` constrains `c_openTicketCount`, and an intent participates only
/// if it declares `c: Customer` too. For an intent that does not, the rule
/// speaks about state the operation provably cannot touch, and the frame that
/// would pin it down is not generated either — so the primed symbols in the
/// goal are unconstrained and Z3 can always pick values that break the rule.
/// Demanding the proof anyway fails every unrelated intent (all 64 in
/// `ticket.intent`, which has nine such rules).
///
/// The name-matching is a real limitation, not a design: an intent that
/// modifies a `Customer` under a different parameter name escapes the rule.
/// It predates this function — the encoder has always resolved these to bare
/// symbols — but a vacuous obligation hid it. See SPEC §5.
fn safety_applies_to(rule: &SafetySource, intent: &IntentDecl) -> bool {
    rule.params.iter().all(|sp| {
        intent
            .params
            .iter()
            .any(|ip| ip.name == sp.name && ip.ty == sp.ty)
    })
}

/// Generate verification conditions from a program.
pub fn generate_vcs(prog: &Program) -> Vec<VerificationCondition> {
    let mut vcs = Vec::new();

    // Collect struct type names for theorem analysis
    let struct_names: HashSet<String> = prog
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Type(t) => Some(t.name.clone()),
            _ => None,
        })
        .collect();

    // struct name -> field names (for D2 frame equalities)
    let struct_fields: HashMap<String, Vec<String>> = prog
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Type(t) => Some((
                t.name.clone(),
                t.fields.iter().map(|f| f.name.clone()).collect(),
            )),
            _ => None,
        })
        .collect();

    // Collect safety rules
    let mut safety_rules: Vec<SafetySource> = Vec::new();
    for decl in &prog.declarations {
        if let Declaration::Safety(s) = &decl.node {
            for (i, inv) in s.invariants.iter().enumerate() {
                safety_rules.push(SafetySource {
                    safety_name: s.name.clone(),
                    index: i + 1,
                    expr: inv.clone(),
                    params: s.params.clone(),
                });
            }
        }
    }

    for decl in &prog.declarations {
        match &decl.node {
            Declaration::Intent(intent) => {
                let mut assumes = Vec::new();
                let mut goals = Vec::new();
                let mut declarations = Vec::new();

                for p in &intent.params {
                    declarations.push(SmtDecl::DeclareConst(p.name.clone(), p.ty.clone()));
                }

                for clause in &intent.clauses {
                    let e = &clause.node.expr;
                    match clause.node.kind {
                        ClauseKind::Require => assumes.push(e.clone()),
                        // Ensures DEFINE post-state — they are assumed
                        ClauseKind::Ensure => assumes.push(e.clone()),
                        ClauseKind::Invariant => {
                            // Pre-state invariant: assumed (unprimed)
                            assumes.push(unprime_expr(e));
                            // Post-state invariant: must be proved
                            goals.push(invariant_goal(e));
                        }
                    }
                }

                // D2 frame semantics: everything outside the frame stays equal.
                if let Some(frame) = intent_frame(intent) {
                    assumes.extend(frame_equalities(intent, &struct_fields, &frame));
                }

                // Safety rules: assume unprimed, prove primed. Only those
                // whose parameters this intent shares — the rest describe
                // state it cannot reach.
                let sr: Vec<SafetySource> = safety_rules
                    .iter()
                    .filter(|rule| safety_applies_to(rule, intent))
                    .cloned()
                    .collect();
                for rule in &sr {
                    assumes.push(unprime_expr(&rule.expr));
                    goals.push(invariant_goal(&rule.expr));
                }

                vcs.push(VerificationCondition {
                    name: intent.name.clone(),
                    kind: VcKind::Intent,
                    declarations: declarations.clone(),
                    assumes,
                    goals,
                    safety_rules: sr.clone(),
                    unsupported: None,
                });

                // D3: for each `require ... else reject` clause, emit a VC
                // checking the reject branch — the violated require plus
                // "no state changes" must be consistent with safety rules.
                let mut req_idx = 0usize;
                for clause in &intent.clauses {
                    if clause.node.kind != ClauseKind::Require {
                        continue;
                    }
                    let this_idx = req_idx;
                    req_idx += 1;
                    if !clause.node.else_reject {
                        continue;
                    }

                    let mut r_assumes: Vec<Spanned<Expr>> = Vec::new();
                    // Other requires still hold; the marked one is violated.
                    let mut ri = 0usize;
                    for other in &intent.clauses {
                        if other.node.kind != ClauseKind::Require {
                            continue;
                        }
                        if ri == this_idx {
                            r_assumes.push(Spanned::new(
                                Expr::UnaryOp(
                                    UnaryOp::Not,
                                    Box::new(Spanned::new(
                                        Expr::Paren(Box::new(other.node.expr.clone())),
                                        other.node.expr.span.clone(),
                                    )),
                                ),
                                other.node.expr.span.clone(),
                            ));
                        } else {
                            r_assumes.push(other.node.expr.clone());
                        }
                        ri += 1;
                    }
                    // Rejection = empty frame: nothing changes.
                    let empty_frame = BTreeSet::new();
                    r_assumes.extend(frame_equalities(intent, &struct_fields, &empty_frame));

                    // Pre-state invariants + safety assumed; primed proved.
                    let mut r_goals = Vec::new();
                    for other in &intent.clauses {
                        if other.node.kind == ClauseKind::Invariant {
                            r_assumes.push(unprime_expr(&other.node.expr));
                            r_goals.push(invariant_goal(&other.node.expr));
                        }
                    }
                    for rule in &sr {
                        r_assumes.push(unprime_expr(&rule.expr));
                        r_goals.push(invariant_goal(&rule.expr));
                    }

                    let clause_id = clause.node.stable_id(&intent.name, this_idx);
                    vcs.push(VerificationCondition {
                        name: format!("{clause_id} (reject branch)"),
                        kind: VcKind::Intent,
                        declarations: declarations.clone(),
                        assumes: r_assumes,
                        goals: r_goals,
                        safety_rules: sr.clone(),
                        unsupported: None,
                    });
                }
            }
            Declaration::Theorem(thm) => {
                // Check if theorem references struct-typed quantifier variables
                let has_struct_quantifiers = uses_struct_quantifiers(&thm.body, &struct_names);
                vcs.push(VerificationCondition {
                    name: thm.name.clone(),
                    kind: VcKind::Theorem,
                    declarations: Vec::new(),
                    assumes: Vec::new(),
                    goals: vec![thm.body.clone()],
                    safety_rules: Vec::new(),
                    unsupported: if has_struct_quantifiers {
                        Some("theorem uses struct-typed quantifiers (requires intent expansion, not yet implemented)".to_string())
                    } else {
                        None
                    },
                });
            }
            _ => {}
        }
    }

    vcs
}

/// Check if an expression contains forall/exists with struct-typed variables.
fn uses_struct_quantifiers(expr: &Spanned<Expr>, struct_names: &HashSet<String>) -> bool {
    match &expr.node {
        Expr::Forall(vars, body) | Expr::Exists(vars, body) => {
            let has_struct = vars.iter().any(|v| match &v.ty {
                TypeExpr::Named(name) => struct_names.contains(name),
                _ => false,
            });
            has_struct || uses_struct_quantifiers(body, struct_names)
        }
        Expr::BinOp(l, _, r) => {
            uses_struct_quantifiers(l, struct_names) || uses_struct_quantifiers(r, struct_names)
        }
        Expr::UnaryOp(_, o) => uses_struct_quantifiers(o, struct_names),
        Expr::Paren(inner) | Expr::Prime(inner) => uses_struct_quantifiers(inner, struct_names),
        Expr::IfThenElse(c, t, e) => {
            uses_struct_quantifiers(c, struct_names)
                || uses_struct_quantifiers(t, struct_names)
                || uses_struct_quantifiers(e, struct_names)
        }
        Expr::Call(_, args) => args
            .iter()
            .any(|a| uses_struct_quantifiers(a, struct_names)),
        Expr::FieldAccess(base, _) => uses_struct_quantifiers(base, struct_names),
        Expr::Index(base, idx) => {
            uses_struct_quantifiers(base, struct_names)
                || uses_struct_quantifiers(idx, struct_names)
        }
        _ => false,
    }
}
