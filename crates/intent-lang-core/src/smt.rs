use std::collections::{HashMap, HashSet};

use intent_lang_syntax::ast::*;
use z3::{Config, SatResult, Solver, with_z3_config};

use crate::vcgen::{VcKind, VerificationCondition};

// ── Result type ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum VerifyResult {
    Verified,
    Failed { counterexample: String },
    Unknown { reason: String },
    Error { message: String },
    /// The intent's own clauses (require ∧ ensure ∧ invariant ∧ frame) are
    /// unsatisfiable: no state can ever satisfy this requirement. Without
    /// this check a contradictory intent would be *vacuously* "verified"
    /// (V0020, rfc-modeling-integrity D1).
    SelfContradictory,
}

// ── Type info for encoding ───────────────────────────────────

#[derive(Debug, Clone)]
struct FieldInfo {
    sort: String,
}

struct TypeInfo {
    /// struct_name -> { field_name -> smt_sort }
    structs: HashMap<String, HashMap<String, FieldInfo>>,
    enums: HashSet<String>,
}

impl TypeInfo {
    fn from_program(prog: &Program) -> Self {
        let mut structs = HashMap::new();
        let mut enums = HashSet::new();
        for decl in &prog.declarations {
            match &decl.node {
                Declaration::Type(t) => {
                    let mut fields = HashMap::new();
                    for f in &t.fields {
                        fields.insert(
                            f.name.clone(),
                            FieldInfo {
                                sort: type_expr_to_smt(&f.ty),
                            },
                        );
                    }
                    structs.insert(t.name.clone(), fields);
                }
                Declaration::Enum(e) => {
                    enums.insert(e.name.clone());
                }
                _ => {}
            }
        }
        TypeInfo { structs, enums }
    }

    fn field_sort(&self, struct_name: &str, field: &str) -> Option<&str> {
        self.structs
            .get(struct_name)
            .and_then(|f| f.get(field))
            .map(|fi| fi.sort.as_str())
    }
}

fn type_expr_to_smt(te: &TypeExpr) -> String {
    match te {
        TypeExpr::Named(n) => match n.as_str() {
            "Int" => "Int".to_string(),
            "Bool" => "Bool".to_string(),
            "String" => "String".to_string(),
            other => other.to_string(),
        },
        TypeExpr::Qualified(module, name) => {
            // Use mangled name for SMT: module_Type
            let full = format!("{module}_{name}");
            match full.as_str() {
                "Int" | "Bool" | "String" => full,
                _ => full,
            }
        }
        TypeExpr::Generic(name, args) => match name.as_str() {
            "Seq" => format!("(Array Int {})", type_expr_to_smt(&args[0])),
            "Set" => format!("(Array {} Bool)", type_expr_to_smt(&args[0])),
            _ => name.clone(),
        },
    }
}

// ── SMT-LIB2 encoder ────────────────────────────────────────

pub struct SmtEncoder {
    /// Lines of SMT output, built top-down.
    lines: Vec<String>,
    declared: HashSet<String>,
    type_info: TypeInfo,
    /// param_name -> type_name (for struct parameters)
    param_types: HashMap<String, String>,
}

impl SmtEncoder {
    pub fn new(prog: &Program) -> Self {
        Self {
            lines: Vec::new(),
            declared: HashSet::new(),
            type_info: TypeInfo::from_program(prog),
            param_types: HashMap::new(),
        }
    }

    pub fn encode_vc(&mut self, vc: &VerificationCondition, prog: &Program) {
        self.encode_vc_inner(vc, prog, true);
    }

    /// Encode only the assumes (no negated goal). `check-sat` on this
    /// answers "can this intent's clauses ever hold simultaneously?" —
    /// used for the vacuity check (D1) and witness solving (D8).
    pub fn encode_assumes_only(&mut self, vc: &VerificationCondition, prog: &Program) {
        self.encode_vc_inner(vc, prog, false);
    }

    fn encode_vc_inner(&mut self, vc: &VerificationCondition, prog: &Program, with_goals: bool) {
        self.lines.clear();
        self.declared.clear();
        self.param_types.clear();

        self.emit("(set-logic ALL)");

        // Declare enum datatypes
        for decl in &prog.declarations {
            if let Declaration::Enum(e) = &decl.node {
                let variants: Vec<String> = e.variants.iter().map(|v| format!("({v})")).collect();
                self.emit(&format!(
                    "(declare-datatype {} ({}))",
                    e.name,
                    variants.join(" ")
                ));
                self.declared.insert(e.name.clone());
            }
        }

        // Flatten struct parameters into individual field constants
        for d in &vc.declarations {
            match d {
                crate::vcgen::SmtDecl::DeclareConst(name, ty) => {
                    let type_name = match ty {
                        TypeExpr::Named(n) => n.clone(),
                        TypeExpr::Qualified(m, n) => format!("{m}.{n}"),
                        TypeExpr::Generic(n, _) => n.clone(),
                    };

                    if self.type_info.structs.contains_key(&type_name) {
                        let field_entries: Vec<(String, String)> = self.type_info.structs
                            [&type_name]
                            .iter()
                            .map(|(fname, fi)| (fname.clone(), fi.sort.clone()))
                            .collect();
                        self.param_types.insert(name.clone(), type_name.clone());
                        for (field_name, sort) in &field_entries {
                            let const_name = format!("{name}_{field_name}");
                            self.declare_const(&const_name, sort);
                            self.declare_const(&format!("{const_name}_prime"), sort);
                        }
                    } else {
                        let sort = type_expr_to_smt(ty);
                        self.declare_const(name, &sort);
                        self.declare_const(&format!("{name}_prime"), &sort);
                    }
                }
                _ => {}
            }
        }

        match vc.kind {
            VcKind::Intent => {
                // Assert assumes (requires + ensures + invariant-pre + frame)
                for e in &vc.assumes {
                    let smt = self.expr_to_smt(e);
                    self.emit(&format!("(assert {smt})"));
                }
                // Negate conjunction of goals (invariants-post)
                if with_goals && !vc.goals.is_empty() {
                    let goal_parts: Vec<String> =
                        vc.goals.iter().map(|e| self.expr_to_smt(e)).collect();
                    let conj = if goal_parts.len() == 1 {
                        goal_parts[0].clone()
                    } else {
                        format!("(and {})", goal_parts.join(" "))
                    };
                    self.emit(&format!("(assert (not {conj}))"));
                }
            }
            VcKind::Theorem => {
                // Theorems with struct-typed quantifiers are not yet supported.
                // For now, we skip encoding if struct types appear in quantifiers.
                if let Some(body) = vc.goals.first() {
                    let smt = self.expr_to_smt(body);
                    if with_goals {
                        self.emit(&format!("(assert (not {smt}))"));
                    } else {
                        self.emit(&format!("(assert {smt})"));
                    }
                }
            }
        }

        self.emit("(check-sat)");
        // get-model only makes sense when sat; Z3 will error on unsat.
        // We handle this by checking the first line of output.
    }

    pub fn get_output(&self) -> String {
        self.lines.join("\n")
    }

    fn declare_const(&mut self, name: &str, sort: &str) {
        if self.declared.insert(name.to_string()) {
            self.emit(&format!("(declare-const {name} {sort})"));
        }
    }

    fn emit(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    // ── Expression encoding ──────────────────────────────

    fn expr_to_smt(&mut self, expr: &Spanned<Expr>) -> String {
        match &expr.node {
            Expr::IntLit(v) => {
                if *v < 0 {
                    format!("(- {})", -v)
                } else {
                    v.to_string()
                }
            }
            Expr::BoolLit(b) => b.to_string(),
            Expr::StringLit(s) => format!("\"{s}\""),

            Expr::Ident(name) => {
                // Check if it's an enum variant
                for decl_name in &self.type_info.enums {
                    if let Some(fields) = self.type_info.structs.get(decl_name) {
                        // not an enum
                        let _ = fields;
                    }
                }
                self.sanitize(name)
            }

            Expr::Prime(inner) => {
                let base = self.expr_to_smt(inner);
                format!("{base}_prime")
            }

            Expr::FieldAccess(base, field) => {
                let b = self.expr_to_smt(base);
                let name = format!("{b}_{field}");
                // Ensure this field constant is declared
                if !self.declared.contains(&name) {
                    // Try to find the sort from type info
                    let sort = self
                        .resolve_field_sort(&b, field)
                        .unwrap_or_else(|| "Int".to_string());
                    self.declare_const(&name, &sort);
                    // Also declare primed
                    self.declare_const(&format!("{name}_prime"), &sort);
                }
                name
            }

            Expr::Index(base, idx) => {
                let b = self.expr_to_smt(base);
                let i = self.expr_to_smt(idx);
                format!("(select {b} {i})")
            }

            Expr::BinOp(lhs, op, rhs) => {
                let l = self.expr_to_smt(lhs);
                let r = self.expr_to_smt(rhs);
                match op {
                    BinOp::Add => format!("(+ {l} {r})"),
                    BinOp::Sub => format!("(- {l} {r})"),
                    BinOp::Mul => format!("(* {l} {r})"),
                    BinOp::Div => format!("(div {l} {r})"),
                    BinOp::Mod => format!("(mod {l} {r})"),
                    BinOp::Eq => format!("(= {l} {r})"),
                    BinOp::Neq => format!("(not (= {l} {r}))"),
                    BinOp::Lt => format!("(< {l} {r})"),
                    BinOp::Le => format!("(<= {l} {r})"),
                    BinOp::Gt => format!("(> {l} {r})"),
                    BinOp::Ge => format!("(>= {l} {r})"),
                    BinOp::And => format!("(and {l} {r})"),
                    BinOp::Or => format!("(or {l} {r})"),
                    BinOp::Implies => format!("(=> {l} {r})"),
                }
            }

            Expr::UnaryOp(op, operand) => {
                let o = self.expr_to_smt(operand);
                match op {
                    UnaryOp::Not => format!("(not {o})"),
                    UnaryOp::Neg => format!("(- {o})"),
                }
            }

            Expr::IfThenElse(c, t, e) => {
                let sc = self.expr_to_smt(c);
                let st = self.expr_to_smt(t);
                let se = self.expr_to_smt(e);
                format!("(ite {sc} {st} {se})")
            }

            Expr::Forall(vars, body) => {
                let bindings: Vec<String> = vars
                    .iter()
                    .map(|v| {
                        let sort = type_expr_to_smt(&v.ty);
                        format!("({} {})", self.sanitize(&v.name), sort)
                    })
                    .collect();
                let b = self.expr_to_smt(body);
                format!("(forall ({}) {})", bindings.join(" "), b)
            }

            Expr::Exists(vars, body) => {
                let bindings: Vec<String> = vars
                    .iter()
                    .map(|v| {
                        let sort = type_expr_to_smt(&v.ty);
                        format!("({} {})", self.sanitize(&v.name), sort)
                    })
                    .collect();
                let b = self.expr_to_smt(body);
                format!("(exists ({}) {})", bindings.join(" "), b)
            }

            Expr::Call(name, args) => {
                if args.is_empty() {
                    self.sanitize(name)
                } else {
                    let smt_args: Vec<String> = args.iter().map(|a| self.expr_to_smt(a)).collect();
                    format!("({} {})", self.sanitize(name), smt_args.join(" "))
                }
            }

            Expr::Paren(inner) => self.expr_to_smt(inner),
        }
    }

    fn resolve_field_sort(&self, base_var: &str, field: &str) -> Option<String> {
        // Find what struct type this variable is
        if let Some(type_name) = self.param_types.get(base_var) {
            return self
                .type_info
                .field_sort(type_name, field)
                .map(|s| s.to_string());
        }
        // Try to find nested: e.g., base_var = "home_frontDoor", field = "locked"
        // Walk through all structs to find a match
        for (_, fields) in &self.type_info.structs {
            for (fname, fi) in fields {
                let compound = format!("{base_var}_{fname}");
                if compound == format!("{base_var}_{field}") {
                    return Some(fi.sort.clone());
                }
            }
        }
        None
    }

    fn sanitize(&self, name: &str) -> String {
        match name {
            "and" | "or" | "not" | "true" | "false" | "ite" | "let" | "forall" | "exists" => {
                format!("intent_{name}")
            }
            _ => name.to_string(),
        }
    }
}

// ── Z3 invocation (in-process, statically linked via `z3` crate) ──

/// Strip solver commands from SMT-LIB2; `Solver::from_string` only accepts declarations/assertions.
fn smt_for_solver(smt_input: &str) -> String {
    smt_input
        .lines()
        .filter(|line| {
            let t = line.trim();
            !t.starts_with("(check-sat)") && !t.starts_with("(get-model)")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Count the `(assert ...)` forms in an SMT-LIB2 text. `SmtEncoder::emit`
/// writes one form per line, so counting lines is exact for our own output.
fn count_asserts(smt: &str) -> usize {
    smt.lines()
        .filter(|line| line.trim_start().starts_with("(assert "))
        .count()
}

/// Load `smt` into a fresh solver, refusing to answer if Z3 did not take every
/// assertion we wrote.
///
/// `Solver::from_string` returns `()`. When a form is malformed — in practice,
/// an application of a symbol the encoder never declared — Z3 reports it
/// through the error handler, **drops that form, and keeps parsing**. The
/// solver is then left holding fewer constraints than we wrote, and an
/// under-constrained solver says `sat`. That answer travels all the way out as
/// a confident verdict: `run_z3` turns it into a counterexample and `run_z3_sat`
/// into a witness, while a query with nothing left to prove comes back
/// "verified". So an encoder bug used to surface as a wrong answer rather than
/// as a failure — the one outcome a verifier must never produce.
///
/// Comparing counts cannot say which symbol Z3 choked on, but it does let us
/// refuse to answer, which is the property that matters.
fn load_solver(smt: &str) -> Result<Solver, String> {
    let solver = Solver::new();
    solver.from_string(smt.as_bytes());

    let written = count_asserts(smt);
    let ingested = solver.get_assertions().len();
    if ingested != written {
        return Err(format!(
            "Z3 rejected part of the generated SMT-LIB2 ({written} assertion(s) written, \
             {ingested} accepted), so no verdict can be trusted. This is a defect in \
             intent-lang's encoder rather than in your requirements — re-run with \
             `--show-smt` to see what was emitted."
        ));
    }
    Ok(solver)
}

pub fn run_z3(smt_input: &str) -> VerifyResult {
    let mut cfg = Config::new();
    cfg.set_timeout_msec(5_000);
    let smt = smt_for_solver(smt_input);

    with_z3_config(&cfg, || {
        let solver = match load_solver(&smt) {
            Ok(solver) => solver,
            Err(message) => return VerifyResult::Error { message },
        };

        match solver.check() {
            SatResult::Unsat => VerifyResult::Verified,
            SatResult::Sat => {
                let counterexample = solver
                    .get_model()
                    .map(|model| parse_z3_model(&model.to_string()))
                    .unwrap_or_default();
                VerifyResult::Failed { counterexample }
            }
            SatResult::Unknown => VerifyResult::Unknown {
                reason: "Z3 returned unknown (timeout or undecidable fragment)".to_string(),
            },
        }
    })
}

/// Parse Z3 model output into (flattened_name, value) pairs.
/// Handles both the `z3` crate's `name -> value` line format and
/// SMT-LIB `(model (define-fun sender_balance () Int 100) ...)`.
/// Names keep the flattened form (`sender_balance`, `sender_balance_prime`).
pub fn parse_z3_model_pairs(raw: &str) -> Vec<(String, String)> {
    // Format 1: `name -> value` per line (z3 crate Display).
    if raw.contains(" -> ") {
        let mut pairs = Vec::new();
        for line in raw.lines() {
            if let Some((name, value)) = line.split_once(" -> ") {
                let name = name.trim();
                if name.is_empty() || name.contains(' ') {
                    continue;
                }
                pairs.push((name.to_string(), extract_smt_value(value.trim())));
            }
        }
        return pairs;
    }

    // Format 2: SMT-LIB define-fun.
    let mut pairs = Vec::new();
    let mut remaining = raw;
    while let Some(pos) = remaining.find("define-fun ") {
        remaining = &remaining[pos + 11..];
        // Parse name
        let name_end = remaining
            .find(|c: char| c.is_whitespace())
            .unwrap_or(remaining.len());
        let name = remaining[..name_end].to_string();
        remaining = &remaining[name_end..];

        // Skip past "() <sort> " to get value
        if let Some(paren_pos) = remaining.find("()") {
            remaining = &remaining[paren_pos + 2..].trim_start();
            // Skip sort
            let sort_end = remaining
                .find(|c: char| c.is_whitespace())
                .unwrap_or(remaining.len());
            remaining = &remaining[sort_end..].trim_start();
            let value = extract_smt_value(remaining);
            pairs.push((name, value));
        }
    }
    pairs
}

/// Human-readable rendering of a model (pre-state values only).
fn parse_z3_model(raw: &str) -> String {
    let assignments: Vec<String> = parse_z3_model_pairs(raw)
        .into_iter()
        .filter(|(name, _)| !name.ends_with("_prime"))
        .map(|(name, value)| format!("{} = {}", name.replace('_', "."), value))
        .collect();

    if assignments.is_empty() {
        raw.to_string()
    } else {
        assignments.join(", ")
    }
}

/// Extract a value from SMT-LIB2 output (handles negative numbers like (- 1))
fn extract_smt_value(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('(') {
        // Could be (- N) for negative number or complex expression
        if let Some(close) = find_matching_paren(s) {
            let inner = &s[1..close].trim();
            if inner.starts_with("- ") {
                return format!("-{}", inner[2..].trim());
            }
            return inner.to_string();
        }
        // Fallback
        s.chars().take(30).collect()
    } else {
        // Simple value: number, true, false, string literal
        s.split(|c: char| c == ')' || c == '\n')
            .next()
            .unwrap_or(s)
            .trim()
            .to_string()
    }
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ── Satisfiability / witness solving (D1 vacuity + D8 witnesses) ──

/// Outcome of solving a plain satisfiability query (no negated goal).
#[derive(Debug, Clone)]
pub enum SatOutcome {
    /// Satisfiable, with the model as (flattened_name, value) pairs.
    Sat { model: Vec<(String, String)> },
    Unsat,
    Unknown { reason: String },
    /// The query never reached the solver intact — see [`load_solver`]. Kept
    /// separate from `Unknown` because a caller may reasonably act on a
    /// timeout (retry, widen the budget) while this one means the tool is
    /// broken and no answer from this query may be used.
    Error { message: String },
}

/// Run Z3 on an SMT input expecting a satisfiability query (not a proof).
pub fn run_z3_sat(smt_input: &str) -> SatOutcome {
    let mut cfg = Config::new();
    cfg.set_timeout_msec(5_000);
    let smt = smt_for_solver(smt_input);

    with_z3_config(&cfg, || {
        // A dropped assertion would make this query answer `Sat` on a weaker
        // constraint set than asked for: a bogus witness, or — via the
        // anti-vacuity path in `verify_vc` — a missed `SelfContradictory`.
        let solver = match load_solver(&smt) {
            Ok(solver) => solver,
            Err(message) => return SatOutcome::Error { message },
        };

        match solver.check() {
            SatResult::Sat => {
                let model = solver
                    .get_model()
                    .map(|m| parse_z3_model_pairs(&m.to_string()))
                    .unwrap_or_default();
                SatOutcome::Sat { model }
            }
            SatResult::Unsat => SatOutcome::Unsat,
            SatResult::Unknown => SatOutcome::Unknown {
                reason: "Z3 returned unknown (timeout or undecidable fragment)".to_string(),
            },
        }
    })
}

/// Check whether an intent's assumes (require ∧ ensure ∧ invariant ∧ frame)
/// are satisfiable. `Unsat` means the intent is self-contradictory (V0020).
/// On `Sat`, the model doubles as a happy-path witness (D8).
pub fn solve_assumes(vc: &VerificationCondition, prog: &Program) -> SatOutcome {
    let mut encoder = SmtEncoder::new(prog);
    encoder.encode_assumes_only(vc, prog);
    run_z3_sat(&encoder.get_output())
}

/// Solve an arbitrary constraint set over an intent's parameter space:
/// the intent's declarations are used, but only `constraints` are asserted.
/// Used by acceptance witness generation (happy / negative / boundary).
pub fn solve_constraints(
    vc: &VerificationCondition,
    prog: &Program,
    constraints: &[Spanned<Expr>],
) -> SatOutcome {
    let mut constrained = vc.clone();
    constrained.assumes = constraints.to_vec();
    constrained.goals = Vec::new();
    let mut encoder = SmtEncoder::new(prog);
    encoder.encode_assumes_only(&constrained, prog);
    run_z3_sat(&encoder.get_output())
}

/// Verify a single VC: encode → call Z3 → return result.
///
/// For intents, a "verified" outcome is additionally protected against
/// vacuous truth: if the assumes themselves are unsatisfiable, the
/// implication `assumes → goals` holds trivially and proves nothing.
/// Such intents are reported as `SelfContradictory` instead (D1).
pub fn verify_vc(vc: &VerificationCondition, prog: &Program) -> VerifyResult {
    // Intents with no goals are trivially verified (nothing to prove) —
    // but still subject to the vacuity check below.
    let base = if vc.kind == VcKind::Intent && vc.goals.is_empty() {
        VerifyResult::Verified
    } else {
        let mut encoder = SmtEncoder::new(prog);
        encoder.encode_vc(vc, prog);
        run_z3(&encoder.get_output())
    };

    // Anti-vacuity second check: only green results need forgery protection.
    if vc.kind == VcKind::Intent && matches!(base, VerifyResult::Verified) {
        match solve_assumes(vc, prog) {
            SatOutcome::Unsat => return VerifyResult::SelfContradictory,
            // An intent with no goals is "verified" without consulting Z3 at
            // all, so this is the only query behind that verdict. If it did
            // not survive the parser we cannot claim the intent is even
            // satisfiable, let alone verified.
            SatOutcome::Error { message } => return VerifyResult::Error { message },
            SatOutcome::Sat { .. } | SatOutcome::Unknown { .. } => {}
        }
    }

    base
}
