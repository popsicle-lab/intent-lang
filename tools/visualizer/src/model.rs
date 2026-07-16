//! Structured "detail" data for the interactive HTML page.
//!
//! The Mermaid diagrams (goal graph, state machine) and the plain-HTML
//! tables (safety rules, operations) only carry a node's *name*. This module
//! builds the full contract behind each name — require/ensure clauses,
//! rationale/measure/stakeholders, invariants, source line — once, so the
//! page's side panel can look anything up by name without re-parsing.
//!
//! Clause and invariant text is sliced directly out of the original source
//! by byte span rather than re-printed from the AST, so operators like
//! `==>` and parenthesization always match what the author wrote.

use crate::goal_graph::{doc_of, goal_mark, GoalKind};
use intent_lang_syntax::ast::*;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct ClauseView {
    pub kind: &'static str,
    pub label: Option<String>,
    pub text: String,
    pub else_reject: bool,
}

#[derive(Debug, Serialize)]
pub struct RealizerRef {
    pub name: String,
    pub kind: &'static str,
}

#[derive(Debug, Serialize)]
pub struct GoalRef {
    pub name: String,
    pub kind: Option<&'static str>,
    pub group: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IntentView {
    pub name: String,
    pub doc: Option<String>,
    pub line: usize,
    pub params: Vec<String>,
    pub modifies: Vec<String>,
    pub clauses: Vec<ClauseView>,
    /// Goals that list this intent in `realized_by` (back-reference for the
    /// panel's "属于目标" cross-links).
    pub goals: Vec<GoalRef>,
}

#[derive(Debug, Serialize)]
pub struct SafetyView {
    pub name: String,
    pub line: usize,
    pub params: Vec<String>,
    pub invariants: Vec<String>,
    pub goals: Vec<GoalRef>,
}

#[derive(Debug, Serialize)]
pub struct GoalView {
    pub name: String,
    pub kind: Option<&'static str>,
    pub group: Option<String>,
    pub rationale: Option<String>,
    pub measure: Option<String>,
    pub stakeholder: Vec<String>,
    pub doc: Option<String>,
    pub realized_by: Vec<RealizerRef>,
    pub line: usize,
}

#[derive(Debug, Serialize, Default)]
pub struct DocModel {
    pub intents: BTreeMap<String, IntentView>,
    pub goals: BTreeMap<String, GoalView>,
    pub safety: BTreeMap<String, SafetyView>,
}

fn line_of(source: &str, byte_offset: usize) -> usize {
    let end = byte_offset.min(source.len());
    source.as_bytes()[..end].iter().filter(|&&b| b == b'\n').count() + 1
}

fn slice<'a>(source: &'a str, span: &Span) -> &'a str {
    source
        .get(span.start..span.end)
        .unwrap_or("")
        .trim()
}

fn kind_label(kind: GoalKind) -> &'static str {
    match kind {
        GoalKind::Capability => "capability",
        GoalKind::Guardrail => "guardrail",
    }
}

pub fn build_doc_model(program: &Program, source: &str) -> DocModel {
    let mut model = DocModel::default();

    // Which realizer names are intents vs. safety rules (for resolving
    // `realized_by` references without guessing).
    let mut realizer_kind: BTreeMap<&str, &'static str> = BTreeMap::new();
    for decl in &program.declarations {
        match &decl.node {
            Declaration::Intent(i) => {
                realizer_kind.insert(i.name.as_str(), "intent");
            }
            Declaration::Safety(s) => {
                realizer_kind.insert(s.name.as_str(), "safety");
            }
            _ => {}
        }
    }

    // Goals first, so intents/safety can be back-linked to the goals that
    // claim them via `realized_by`.
    let mut goal_refs_by_realizer: BTreeMap<&str, Vec<GoalRef>> = BTreeMap::new();
    for decl in &program.declarations {
        let Declaration::Goal(g) = &decl.node else { continue };
        let mark = goal_mark(g);
        let kind = mark.as_ref().map(|m| kind_label(m.kind));
        let group = mark.as_ref().and_then(|m| m.group.clone());

        let realized_by: Vec<RealizerRef> = g
            .realized_by
            .iter()
            .map(|name| RealizerRef {
                name: name.clone(),
                kind: realizer_kind.get(name.as_str()).copied().unwrap_or("unknown"),
            })
            .collect();

        for name in &g.realized_by {
            goal_refs_by_realizer.entry(name.as_str()).or_default().push(GoalRef {
                name: g.name.clone(),
                kind,
                group: group.clone(),
            });
        }

        model.goals.insert(
            g.name.clone(),
            GoalView {
                name: g.name.clone(),
                kind,
                group,
                rationale: g.rationale.clone(),
                measure: g.measure.clone(),
                stakeholder: g.stakeholder.clone(),
                doc: doc_of(&g.annotations),
                realized_by,
                line: line_of(source, decl.span.start),
            },
        );
    }

    for decl in &program.declarations {
        match &decl.node {
            Declaration::Intent(i) => {
                let params = i
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.ty))
                    .collect();
                let modifies = match &i.modifies {
                    None => Vec::new(),
                    Some(ModifiesSpec::Wildcard) => vec!["*".to_string()],
                    Some(ModifiesSpec::Paths(paths)) => {
                        paths.iter().map(|p| slice(source, &p.span).to_string()).collect()
                    }
                };
                let clauses = i
                    .clauses
                    .iter()
                    .map(|c| ClauseView {
                        kind: c.node.kind.keyword(),
                        label: c.node.label.clone(),
                        text: slice(source, &c.node.expr.span).to_string(),
                        else_reject: c.node.else_reject,
                    })
                    .collect();
                model.intents.insert(
                    i.name.clone(),
                    IntentView {
                        name: i.name.clone(),
                        doc: doc_of(&i.annotations),
                        line: line_of(source, decl.span.start),
                        params,
                        modifies,
                        clauses,
                        goals: goal_refs_by_realizer.remove(i.name.as_str()).unwrap_or_default(),
                    },
                );
            }
            Declaration::Safety(s) => {
                let params = s.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
                let invariants = s
                    .invariants
                    .iter()
                    .map(|inv| slice(source, &inv.span).to_string())
                    .collect();
                model.safety.insert(
                    s.name.clone(),
                    SafetyView {
                        name: s.name.clone(),
                        line: line_of(source, decl.span.start),
                        params,
                        invariants,
                        goals: goal_refs_by_realizer.remove(s.name.as_str()).unwrap_or_default(),
                    },
                );
            }
            _ => {}
        }
    }

    model
}

impl crate::GraphData for DocModel {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        intent_lang_syntax::parse(src).expect("parse")
    }

    const SRC: &str = r#"
type A { x: Int }

@capability("g")
goal "cap" {
  rationale: "why"
  measure: "how we'd know"
  stakeholder: ["ops"]
  realized_by: [Op, Rule]
}

safety Rule(a: A) {
  invariant a.x >= 0
}

@doc("bumps x")
intent Op(a: A) {
  modifies a.x
  require pos: a.x > 0 else reject
  ensure bumped: a.x' == a.x + 1
}
"#;

    #[test]
    fn captures_intent_contract_and_line() {
        let model = build_doc_model(&parse(SRC), SRC);
        let op = model.intents.get("Op").expect("Op present");
        assert_eq!(op.doc.as_deref(), Some("bumps x"));
        assert_eq!(op.modifies, vec!["a.x".to_string()]);
        assert!(op.clauses.iter().any(|c| c.kind == "require" && c.text.contains("a.x > 0")));
        assert!(op.clauses.iter().any(|c| c.kind == "ensure" && c.text.contains("a.x' == a.x + 1")));
        assert!(op.line > 0);
        assert_eq!(op.goals.len(), 1);
        assert_eq!(op.goals[0].name, "cap");
    }

    #[test]
    fn captures_safety_invariant_and_back_link() {
        let model = build_doc_model(&parse(SRC), SRC);
        let rule = model.safety.get("Rule").expect("Rule present");
        assert!(rule.invariants[0].contains("a.x >= 0"));
        assert_eq!(rule.goals[0].name, "cap");
    }

    #[test]
    fn captures_goal_metadata() {
        let model = build_doc_model(&parse(SRC), SRC);
        let cap = model.goals.get("cap").expect("cap present");
        assert_eq!(cap.kind, Some("capability"));
        assert_eq!(cap.group.as_deref(), Some("g"));
        assert_eq!(cap.rationale.as_deref(), Some("why"));
        assert_eq!(cap.realized_by.len(), 2);
        assert!(cap.realized_by.iter().any(|r| r.name == "Op" && r.kind == "intent"));
        assert!(cap.realized_by.iter().any(|r| r.name == "Rule" && r.kind == "safety"));
    }
}
