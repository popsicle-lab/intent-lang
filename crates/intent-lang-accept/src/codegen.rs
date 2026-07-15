//! Deterministic pytest generation (D6): testspec + Z3 witnesses +
//! binding → test code. No LLM anywhere in this path — assertion logic
//! is a mechanical, auditable translation of requirement clauses.

use std::collections::BTreeMap;

use intent_lang_core::analysis::{clause_index, ClauseInfo, Executability};
use intent_lang_core::vcgen::intent_frame;
use intent_lang_core::witness::{program_witnesses, ScenarioWitness, WitnessKind};
use intent_lang_syntax::ast::*;
use serde::{Deserialize, Serialize};

use crate::binding::{fill_template, Binding};

// ── Manifest: test ↔ clause mapping, written next to the tests ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub kind: String,
    pub intent_file: String,
    pub binding_file: String,
    pub adapter: String,
    pub tests: Vec<TestEntry>,
    pub manual_items: Vec<ManualItem>,
    /// All machine clause IDs covered by at least one test.
    pub machine_clause_ids: Vec<String>,
    /// goal name → clause IDs (via realized_by → intent clauses).
    pub goal_clauses: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEntry {
    pub name: String,
    pub scenario: String,
    pub clause_ids: Vec<String>,
    pub expect_reject: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualItem {
    pub clause_id: String,
    pub reason: String,
}

pub struct GenOutput {
    pub pytest_code: String,
    pub manifest: Manifest,
}

// ── Python expression translation ────────────────────────────

struct PyCtx<'a> {
    binding: &'a Binding,
    /// param name → struct type name (None for scalars)
    param_types: BTreeMap<String, Option<String>>,
    enum_variants: Vec<String>,
}

impl<'a> PyCtx<'a> {
    /// dotted state path `sender.balance` → binding key `Account.balance`
    fn state_key(&self, path: &str) -> Option<String> {
        let (param, field) = path.split_once('.')?;
        let ty = self.param_types.get(param)?.as_ref()?;
        Some(format!("{ty}.{field}"))
    }

    fn state_readable(&self, path: &str) -> bool {
        self.state_key(path)
            .map(|k| self.binding.state.contains_key(&k))
            .unwrap_or(false)
    }
}

fn flatten_path(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident(n) => Some(n.clone()),
        Expr::FieldAccess(base, field) => Some(format!("{}.{field}", flatten_path(&base.node)?)),
        Expr::Paren(inner) => flatten_path(&inner.node),
        _ => None,
    }
}

/// Translate a clause expression into a Python expression over
/// `pre[...]`, `post[...]` snapshots and parameter variables.
/// Err(reason) ⇒ the clause is not machine-checkable with this binding.
fn expr_to_py(ctx: &PyCtx, e: &Spanned<Expr>) -> Result<String, String> {
    match &e.node {
        Expr::IntLit(v) => Ok(v.to_string()),
        Expr::BoolLit(b) => Ok(if *b { "True" } else { "False" }.to_string()),
        Expr::StringLit(s) => Ok(format!("{s:?}")),
        Expr::Ident(n) => {
            if ctx.param_types.contains_key(n) {
                Ok(py_ident(n))
            } else if ctx.enum_variants.contains(n) {
                // Enums are represented as their variant name (string).
                Ok(format!("{n:?}"))
            } else {
                Err(format!("unknown identifier `{n}`"))
            }
        }
        Expr::Prime(inner) => {
            let path = flatten_path(&inner.node)
                .ok_or_else(|| "primed non-path expression".to_string())?;
            if ctx.state_readable(&path) {
                Ok(format!("post[{path:?}]"))
            } else {
                Err(format!("state `{path}` not observable in binding"))
            }
        }
        Expr::FieldAccess(..) => {
            let path = flatten_path(&e.node).ok_or_else(|| "complex field access".to_string())?;
            if ctx.state_readable(&path) {
                Ok(format!("pre[{path:?}]"))
            } else {
                Err(format!("state `{path}` not observable in binding"))
            }
        }
        Expr::BinOp(l, op, r) => {
            let pl = expr_to_py(ctx, l)?;
            let pr = expr_to_py(ctx, r)?;
            let py = match op {
                BinOp::Add => format!("({pl} + {pr})"),
                BinOp::Sub => format!("({pl} - {pr})"),
                BinOp::Mul => format!("({pl} * {pr})"),
                // SMT `div` is euclidean; Python `//` floors. They agree
                // for non-negative operands, which covers our Int models.
                BinOp::Div => format!("({pl} // {pr})"),
                BinOp::Mod => format!("({pl} % {pr})"),
                BinOp::Eq => format!("({pl} == {pr})"),
                BinOp::Neq => format!("({pl} != {pr})"),
                BinOp::Lt => format!("({pl} < {pr})"),
                BinOp::Le => format!("({pl} <= {pr})"),
                BinOp::Gt => format!("({pl} > {pr})"),
                BinOp::Ge => format!("({pl} >= {pr})"),
                BinOp::And => format!("({pl} and {pr})"),
                BinOp::Or => format!("({pl} or {pr})"),
                BinOp::Implies => format!("((not {pl}) or {pr})"),
            };
            Ok(py)
        }
        Expr::UnaryOp(op, o) => {
            let po = expr_to_py(ctx, o)?;
            Ok(match op {
                UnaryOp::Not => format!("(not {po})"),
                UnaryOp::Neg => format!("(-{po})"),
            })
        }
        Expr::IfThenElse(c, t, el) => {
            let pc = expr_to_py(ctx, c)?;
            let pt = expr_to_py(ctx, t)?;
            let pe = expr_to_py(ctx, el)?;
            Ok(format!("({pt} if {pc} else {pe})"))
        }
        Expr::Paren(inner) => expr_to_py(ctx, inner),
        Expr::Forall(..) | Expr::Exists(..) => Err("quantifier (D7: manual)".to_string()),
        Expr::Call(name, _) => Err(format!("function call `{name}` not translatable")),
        Expr::Index(..) => Err("Seq/Set indexing not supported in M-A1".to_string()),
    }
}

// ── Value rendering ───────────────────────────────────────────

/// Render a Z3 model value (or example literal text) as a Python literal.
fn py_value(raw: &str) -> String {
    let t = raw.trim();
    if t == "true" {
        return "True".to_string();
    }
    if t == "false" {
        return "False".to_string();
    }
    if t.parse::<i64>().is_ok() {
        return t.to_string();
    }
    if t.starts_with('"') && t.ends_with('"') {
        return t.to_string();
    }
    // Enum variant or opaque symbol → string literal.
    format!("{t:?}")
}

fn default_value(field_ty: &TypeExpr, enums: &BTreeMap<String, Vec<String>>) -> String {
    match field_ty {
        TypeExpr::Named(n) => match n.as_str() {
            "Int" => "0".to_string(),
            "Bool" => "False".to_string(),
            "String" => "\"x\"".to_string(),
            other => enums
                .get(other)
                .and_then(|vs| vs.first())
                .map(|v| format!("{v:?}"))
                .unwrap_or_else(|| "None".to_string()),
        },
        _ => "None".to_string(),
    }
}

// ── Generator ─────────────────────────────────────────────────

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// A requirement parameter name may collide with a Python hard keyword
/// (e.g. `from` in `TransferTicket`); emit a safe local identifier so the
/// generated test is valid Python. Dict keys / clause paths are unaffected.
fn py_ident(name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ];
    if KEYWORDS.contains(&name) {
        format!("{name}_")
    } else {
        name.to_string()
    }
}

struct IntentGen<'a> {
    intent: &'a IntentDecl,
    infos: Vec<&'a ClauseInfo>,
    ctx: PyCtx<'a>,
    /// Fields (dotted paths) with a readable state binding.
    readable_paths: Vec<String>,
}

pub fn generate(
    prog: &Program,
    binding: &Binding,
    intent_file: &str,
    binding_file: &str,
    // Directory added to sys.path so the target module resolves —
    // by convention the intent file's directory.
    source_dir: &str,
) -> GenOutput {
    let index = clause_index(prog);
    let witnesses = program_witnesses(prog);

    let enums: BTreeMap<String, Vec<String>> = prog
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Enum(e) => Some((e.name.clone(), e.variants.clone())),
            _ => None,
        })
        .collect();
    let enum_variants: Vec<String> = enums.values().flatten().cloned().collect();
    let struct_fields: BTreeMap<String, Vec<Field>> = prog
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Type(t) => Some((t.name.clone(), t.fields.clone())),
            _ => None,
        })
        .collect();

    let mut code = String::new();
    code.push_str(&format!(
        "# Generated by `intent accept gen` — DO NOT EDIT.\n\
         # Source:  {intent_file}\n\
         # Binding: {binding_file}\n\
         # Assertions are mechanical translations of requirement clauses;\n\
         # each assert message carries the stable clause ID it checks.\n\
         import sys\n\
         sys.path.insert(0, {source_dir:?})\n\
         import pytest\n\
         import {target}\n\n",
        target = binding.meta.target
    ));

    let mut tests: Vec<TestEntry> = Vec::new();
    let mut manual: Vec<ManualItem> = Vec::new();
    let mut machine_ids: Vec<String> = Vec::new();

    for d in &prog.declarations {
        let Declaration::Intent(intent) = &d.node else {
            continue;
        };
        // asis intents are legacy track — skip acceptance.
        if intent_lang_core::analysis::intent_lifecycle(intent)
            == intent_lang_core::analysis::Lifecycle::AsIs
        {
            continue;
        }

        let infos: Vec<&ClauseInfo> = index.iter().filter(|c| c.owner == intent.name).collect();

        let Some(op) = binding.ops.get(&intent.name) else {
            for c in &infos {
                manual.push(ManualItem {
                    clause_id: c.id.clone(),
                    reason: format!("no ops.{} binding — operation not mapped", intent.name),
                });
            }
            continue;
        };

        let param_types: BTreeMap<String, Option<String>> = intent
            .params
            .iter()
            .map(|p| {
                let ty = match &p.ty {
                    TypeExpr::Named(n) if struct_fields.contains_key(n) => Some(n.clone()),
                    _ => None,
                };
                (p.name.clone(), ty)
            })
            .collect();

        let ctx = PyCtx {
            binding,
            param_types,
            enum_variants: enum_variants.clone(),
        };

        // Readable state paths for snapshots.
        let mut readable_paths = Vec::new();
        for p in &intent.params {
            if let TypeExpr::Named(tn) = &p.ty {
                if let Some(fields) = struct_fields.get(tn) {
                    for f in fields {
                        let path = format!("{}.{}", p.name, f.name);
                        if ctx.state_readable(&path) {
                            readable_paths.push(path);
                        }
                    }
                }
            }
        }

        let gen = IntentGen {
            intent,
            infos,
            ctx,
            readable_paths,
        };

        // Quantified clauses → manual items (D7).
        for c in &gen.infos {
            if c.executability == Executability::Manual {
                manual.push(ManualItem {
                    clause_id: c.id.clone(),
                    reason: "quantified clause — manual until `state` semantics land (D7)"
                        .to_string(),
                });
            }
        }

        let iw = witnesses.iter().find(|w| w.intent == intent.name);

        // D5/D8 data source order: example blocks first (human-picked
        // business values), then Z3 witnesses for boundary/negative.
        let mut example_no = 0usize;
        for ed in &prog.declarations {
            let Declaration::Example(ex) = &ed.node else {
                continue;
            };
            if ex.intent != intent.name {
                continue;
            }
            emit_example_test(
                &mut code,
                &mut tests,
                &mut manual,
                &mut machine_ids,
                &gen,
                op,
                ex,
                example_no,
                &struct_fields,
                &enums,
            );
            example_no += 1;
        }

        if let Some(iw) = iw {
            for sc in &iw.scenarios {
                emit_witness_test(
                    &mut code,
                    &mut tests,
                    &mut manual,
                    &mut machine_ids,
                    &gen,
                    op,
                    sc,
                    &struct_fields,
                    &enums,
                );
            }
            for u in &iw.unsolved {
                // Witness solving failures worth surfacing as manual items.
                if !u.starts_with("violates") || u.contains("unknown") {
                    manual.push(ManualItem {
                        clause_id: format!("{}/witness", intent.name),
                        reason: u.clone(),
                    });
                }
            }
        }
    }

    machine_ids.sort();
    machine_ids.dedup();
    manual.sort_by(|a, b| a.clause_id.cmp(&b.clause_id).then(a.reason.cmp(&b.reason)));
    manual.dedup_by(|a, b| a.clause_id == b.clause_id && a.reason == b.reason);

    // goal → clause IDs rollup input (D9).
    let mut goal_clauses: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for d in &prog.declarations {
        if let Declaration::Goal(g) = &d.node {
            let mut ids = Vec::new();
            for owner in &g.realized_by {
                for c in index.iter().filter(|c| &c.owner == owner) {
                    ids.push(c.id.clone());
                }
            }
            goal_clauses.insert(g.name.clone(), ids);
        }
    }

    GenOutput {
        pytest_code: code,
        manifest: Manifest {
            kind: "intent.acceptance_manifest".to_string(),
            intent_file: intent_file.to_string(),
            binding_file: binding_file.to_string(),
            adapter: binding.meta.adapter.clone(),
            tests,
            manual_items: manual,
            machine_clause_ids: machine_ids,
            goal_clauses,
        },
    }
}

/// Build constructor lines for all params, given a value map
/// (dotted path / param name → raw value text).
fn emit_constructors(
    gen: &IntentGen,
    values: &BTreeMap<String, String>,
    struct_fields: &BTreeMap<String, Vec<Field>>,
    enums: &BTreeMap<String, Vec<String>>,
) -> Result<String, String> {
    let mut out = String::new();
    for p in &gen.intent.params {
        match &p.ty {
            TypeExpr::Named(tn) if struct_fields.contains_key(tn) => {
                let tb = gen
                    .ctx
                    .binding
                    .types
                    .get(tn)
                    .ok_or_else(|| format!("no types.{tn} binding"))?;
                let mut fills = BTreeMap::new();
                for f in &struct_fields[tn] {
                    let path = format!("{}.{}", p.name, f.name);
                    let v = values
                        .get(&path)
                        .map(|raw| py_value(raw))
                        .unwrap_or_else(|| default_value(&f.ty, enums));
                    fills.insert(f.name.clone(), v);
                }
                out.push_str(&format!(
                    "    {} = {}\n",
                    py_ident(&p.name),
                    fill_template(&tb.construct, &fills)
                ));
            }
            _ => {
                let v = values
                    .get(&p.name)
                    .map(|raw| py_value(raw))
                    .unwrap_or_else(|| "0".to_string());
                out.push_str(&format!("    {} = {}\n", py_ident(&p.name), v));
            }
        }
    }
    Ok(out)
}

fn emit_snapshot(gen: &IntentGen, var: &str) -> String {
    if gen.readable_paths.is_empty() {
        return format!("    {var} = {{}}\n");
    }
    let mut out = format!("    {var} = {{\n");
    for path in &gen.readable_paths {
        let key = gen.ctx.state_key(path).unwrap();
        let read = &gen.ctx.binding.state[&key].read;
        let (param, _) = path.split_once('.').unwrap();
        let mut fills = BTreeMap::new();
        fills.insert("self".to_string(), py_ident(param));
        out.push_str(&format!(
            "        {path:?}: {},\n",
            fill_template(read, &fills)
        ));
    }
    out.push_str("    }\n");
    out
}

fn emit_call(op: &crate::binding::OpBinding, gen: &IntentGen) -> String {
    let fills: BTreeMap<String, String> = gen
        .intent
        .params
        .iter()
        .map(|p| (p.name.clone(), py_ident(&p.name)))
        .collect();
    fill_template(&op.call, &fills)
}

/// Assert lines for ensures + invariants(post) + frame (D2).
/// Returns Err(reason, clause_id) if some clause is untranslatable.
fn emit_asserts(
    gen: &IntentGen,
    machine_ids: &mut Vec<String>,
    manual: &mut Vec<ManualItem>,
) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut checked_ids = Vec::new();

    let mut i = 0usize;
    for cl in &gen.intent.clauses {
        let info = gen.infos[i];
        i += 1;
        if cl.node.kind == ClauseKind::Require {
            continue;
        }
        if info.executability == Executability::Manual {
            continue; // already a manual item
        }
        match expr_to_py(&gen.ctx, &cl.node.expr) {
            Ok(py) => {
                out.push_str(&format!(
                    "    assert {py}, \"clause {} violated: {}\"\n",
                    info.id, info.text
                ));
                checked_ids.push(info.id.clone());
                machine_ids.push(info.id.clone());
            }
            Err(reason) => {
                manual.push(ManualItem {
                    clause_id: info.id.clone(),
                    reason,
                });
            }
        }
    }

    // D2 frame: readable paths outside the frame must be unchanged.
    if let Some(frame) = intent_frame(gen.intent) {
        let frame_id = format!("{}/frame", gen.intent.name);
        let mut any = false;
        for path in &gen.readable_paths {
            if !frame.contains(path) {
                out.push_str(&format!(
                    "    assert post[{path:?}] == pre[{path:?}], \"clause {frame_id} violated: `{path}` outside `modifies` frame must be unchanged\"\n"
                ));
                any = true;
            }
        }
        if any {
            checked_ids.push(frame_id.clone());
            machine_ids.push(frame_id);
        }
    }

    (out, checked_ids)
}

#[allow(clippy::too_many_arguments)]
fn emit_witness_test(
    code: &mut String,
    tests: &mut Vec<TestEntry>,
    manual: &mut Vec<ManualItem>,
    machine_ids: &mut Vec<String>,
    gen: &IntentGen,
    op: &crate::binding::OpBinding,
    sc: &ScenarioWitness,
    struct_fields: &BTreeMap<String, Vec<Field>>,
    enums: &BTreeMap<String, Vec<String>>,
) -> Option<()> {
    let name = format!("test_{}__{}", slug(&gen.intent.name), slug(&sc.label));

    let ctor = match emit_constructors(gen, &sc.values, struct_fields, enums) {
        Ok(c) => c,
        Err(reason) => {
            for id in &sc.clause_ids {
                manual.push(ManualItem {
                    clause_id: id.clone(),
                    reason: reason.clone(),
                });
            }
            return None;
        }
    };

    match sc.kind {
        WitnessKind::Happy | WitnessKind::Boundary => {
            let (asserts, checked) = emit_asserts(gen, machine_ids, manual);
            if asserts.is_empty() {
                return None;
            }
            code.push_str(&format!("def {name}():\n"));
            code.push_str(&format!("    # scenario: {} (Z3 witness)\n", sc.label));
            code.push_str(&ctor);
            code.push_str(&emit_snapshot(gen, "pre"));
            code.push_str(&format!("    {}\n", emit_call(op, gen)));
            code.push_str(&emit_snapshot(gen, "post"));
            code.push_str(&asserts);
            code.push_str("\n\n");
            tests.push(TestEntry {
                name,
                scenario: sc.label.clone(),
                clause_ids: checked,
                expect_reject: false,
            });
        }
        WitnessKind::ViolatesRequire => {
            if !sc.expect_reject {
                // Caller contract (no `else reject`): behavior unspecified,
                // nothing to test (D3).
                return None;
            }
            let clause_id = sc.clause_ids.first().cloned().unwrap_or_default();
            let reject = op.reject_signal.as_deref().unwrap_or("");
            code.push_str(&format!("def {name}():\n"));
            code.push_str(&format!(
                "    # scenario: {} — expect reject + state unchanged (D3)\n",
                sc.label
            ));
            code.push_str(&ctor);
            code.push_str(&emit_snapshot(gen, "pre"));
            if reject == "raises" {
                let err = op.error_type.as_deref().unwrap_or("Exception");
                code.push_str(&format!("    with pytest.raises({err}):\n"));
                code.push_str(&format!("        {}\n", emit_call(op, gen)));
            } else if let Some(pattern) = reject.strip_prefix("returns_error:") {
                code.push_str(&format!("    result = {}\n", emit_call(op, gen)));
                let mut fills = BTreeMap::new();
                fills.insert("result".to_string(), "result".to_string());
                code.push_str(&format!(
                    "    assert {}, \"clause {clause_id} violated: operation must signal rejection\"\n",
                    fill_template(pattern, &fills)
                ));
            } else {
                manual.push(ManualItem {
                    clause_id,
                    reason: format!(
                        "clause has `else reject` but ops.{} declares no reject_signal",
                        gen.intent.name
                    ),
                });
                // Roll back the emitted def header lines.
                let cut = code.rfind(&format!("def {name}():")).unwrap();
                code.truncate(cut);
                return None;
            }
            code.push_str(&emit_snapshot(gen, "post"));
            code.push_str(&format!(
                "    assert post == pre, \"clause {clause_id} violated: rejected operation must leave state unchanged\"\n"
            ));
            code.push_str("\n\n");
            machine_ids.push(clause_id.clone());
            tests.push(TestEntry {
                name,
                scenario: sc.label.clone(),
                clause_ids: vec![clause_id],
                expect_reject: true,
            });
        }
    }
    Some(())
}

#[allow(clippy::too_many_arguments)]
fn emit_example_test(
    code: &mut String,
    tests: &mut Vec<TestEntry>,
    manual: &mut Vec<ManualItem>,
    machine_ids: &mut Vec<String>,
    gen: &IntentGen,
    op: &crate::binding::OpBinding,
    ex: &ExampleDecl,
    example_no: usize,
    struct_fields: &BTreeMap<String, Vec<Field>>,
    enums: &BTreeMap<String, Vec<String>>,
) {
    let example_id = format!("{}/example[{example_no}]", gen.intent.name);

    // given → value map
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    for b in &ex.given {
        if let Some(path) = flatten_path(&b.path.node) {
            values.insert(path, intent_lang_core::analysis::expr_to_text(&b.value));
        }
    }

    let ctor = match emit_constructors(gen, &values, struct_fields, enums) {
        Ok(c) => c,
        Err(reason) => {
            manual.push(ManualItem {
                clause_id: example_id,
                reason,
            });
            return;
        }
    };

    let name = format!(
        "test_{}__example_{example_no}_{}",
        slug(&gen.intent.name),
        slug(ex.title.as_deref().unwrap_or("untitled"))
    );

    // expect → post-state equality asserts against author's literals (D5).
    let mut expect_asserts = String::new();
    for b in &ex.expect {
        let Expr::Prime(inner) = &b.path.node else {
            continue;
        };
        let Some(path) = flatten_path(&inner.node) else {
            continue;
        };
        if !gen.ctx.state_readable(&path) {
            manual.push(ManualItem {
                clause_id: example_id.clone(),
                reason: format!("state `{path}` not observable in binding"),
            });
            return;
        }
        let v = py_value(&intent_lang_core::analysis::expr_to_text(&b.value));
        // The value is a Python literal that may itself contain double
        // quotes (enum/string variants render as `"Pending"`); the assert
        // message is a double-quoted string, so soften quotes to avoid
        // producing invalid Python.
        let v_msg = v.replace('"', "'");
        expect_asserts.push_str(&format!(
            "    assert post[{path:?}] == {v}, \"clause {example_id} violated: expected {path}' == {v_msg}\"\n"
        ));
    }

    let (clause_asserts, mut checked) = emit_asserts(gen, machine_ids, manual);

    code.push_str(&format!("def {name}():\n"));
    code.push_str(&format!(
        "    # example: {} (D5 — author-picked business values)\n",
        ex.title.as_deref().unwrap_or("untitled")
    ));
    code.push_str(&ctor);
    code.push_str(&emit_snapshot(gen, "pre"));
    code.push_str(&format!("    {}\n", emit_call(op, gen)));
    code.push_str(&emit_snapshot(gen, "post"));
    code.push_str(&expect_asserts);
    code.push_str(&clause_asserts);
    code.push_str("\n\n");

    checked.push(example_id.clone());
    machine_ids.push(example_id);
    tests.push(TestEntry {
        name,
        scenario: format!("example[{example_no}]"),
        clause_ids: checked,
        expect_reject: false,
    });
}
