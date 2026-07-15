/// Intent relationship graph builder
///
/// Builds a graph showing:
/// - Intent declarations and their clauses
/// - Data flow between intents (via shared types)
/// - State transitions (primed variables)

use intent_lang_syntax::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct IntentGraph {
    pub nodes: Vec<IntentNode>,
    pub edges: Vec<IntentEdge>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntentNode {
    pub id: String,
    pub name: String,
    pub params: Vec<String>,
    pub annotations: Vec<String>,
    pub requires: usize,
    pub ensures: usize,
    pub invariants: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntentEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

pub fn build_intent_graph(program: &Program) -> IntentGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Collect all type definitions for reference
    let mut types: HashSet<String> = HashSet::new();
    for decl in &program.declarations {
        if let Declaration::Type(type_decl) = &decl.node {
            types.insert(type_decl.name.clone());
        }
    }

    // Build intent nodes
    let mut intent_params: HashMap<String, Vec<String>> = HashMap::new();

    for decl in &program.declarations {
        if let Declaration::Intent(intent) = &decl.node {
            let mut requires = 0;
            let mut ensures = 0;
            let mut invariants = 0;

            for clause in &intent.clauses {
                match clause.node.kind {
                    ClauseKind::Require => requires += 1,
                    ClauseKind::Ensure => ensures += 1,
                    ClauseKind::Invariant => invariants += 1,
                }
            }

            let params: Vec<String> = intent.params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.ty))
                .collect();

            intent_params.insert(intent.name.clone(), params.clone());

            let annotations: Vec<String> = intent.annotations
                .iter()
                .map(|a| a.name.clone())
                .collect();

            nodes.push(IntentNode {
                id: intent.name.clone(),
                name: intent.name.clone(),
                params,
                annotations,
                requires,
                ensures,
                invariants,
            });
        }
    }

    // Build edges based on shared types (data flow)
    let intent_types: HashMap<String, HashSet<String>> = intent_params
        .iter()
        .map(|(intent, params)| {
            let param_types: HashSet<String> = params
                .iter()
                .filter_map(|p| {
                    // Extract type from "name: Type" format
                    p.split(':')
                        .nth(1)
                        .map(|t| t.trim().to_string())
                })
                .collect();
            (intent.clone(), param_types)
        })
        .collect();

    // Create edges between intents that share types
    for (intent1, types1) in &intent_types {
        for (intent2, types2) in &intent_types {
            if intent1 != intent2 {
                let shared: Vec<_> = types1.intersection(types2).collect();
                if !shared.is_empty() {
                    edges.push(IntentEdge {
                        from: intent1.clone(),
                        to: intent2.clone(),
                        label: Some(shared[0].to_string()),
                    });
                }
            }
        }
    }

    IntentGraph { nodes, edges }
}

pub fn build_verification_flow(program: &Program) -> VerificationFlow {
    // Find the first intent to visualize
    for decl in &program.declarations {
        if let Declaration::Intent(intent) = &decl.node {
            return build_intent_verification_flow(intent);
        }
    }

    VerificationFlow {
        intent_name: "None".to_string(),
        steps: vec![],
    }
}

fn build_intent_verification_flow(intent: &IntentDecl) -> VerificationFlow {
    let mut steps = Vec::new();

    // Step 1: Collect preconditions
    let requires: Vec<String> = intent.clauses
        .iter()
        .filter_map(|c| match c.node.kind {
            ClauseKind::Require => Some(format_expr(&c.node.expr.node)),
            _ => None,
        })
        .collect();

    if !requires.is_empty() {
        steps.push(VerificationStep {
            label: "Preconditions (require)".to_string(),
            content: requires,
        });
    }

    // Step 2: Collect invariants
    let invariants: Vec<String> = intent.clauses
        .iter()
        .filter_map(|c| match c.node.kind {
            ClauseKind::Invariant => Some(format_expr(&c.node.expr.node)),
            _ => None,
        })
        .collect();

    if !invariants.is_empty() {
        steps.push(VerificationStep {
            label: "Invariants (must hold before and after)".to_string(),
            content: invariants,
        });
    }

    // Step 3: Collect postconditions
    let ensures: Vec<String> = intent.clauses
        .iter()
        .filter_map(|c| match c.node.kind {
            ClauseKind::Ensure => Some(format_expr(&c.node.expr.node)),
            _ => None,
        })
        .collect();

    if !ensures.is_empty() {
        steps.push(VerificationStep {
            label: "Postconditions (ensure)".to_string(),
            content: ensures,
        });
    }

    // Step 4: Verification condition
    steps.push(VerificationStep {
        label: "Verification Condition".to_string(),
        content: vec![
            "require ∧ invariant → ensure ∧ invariant'".to_string(),
        ],
    });

    VerificationFlow {
        intent_name: intent.name.clone(),
        steps,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationFlow {
    pub intent_name: String,
    pub steps: Vec<VerificationStep>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationStep {
    pub label: String,
    pub content: Vec<String>,
}

/// Format an expression to a human-readable string (simplified)
fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLit(n) => n.to_string(),
        Expr::BoolLit(b) => b.to_string(),
        Expr::StringLit(s) => format!("\"{}\"", s),
        Expr::Ident(name) => name.clone(),
        Expr::Prime(inner) => format!("{}'", format_expr(&inner.node)),
        Expr::FieldAccess(obj, field) => {
            format!("{}.{}", format_expr(&obj.node), field)
        }
        Expr::BinOp(left, op, right) => {
            format!("{} {} {}", format_expr(&left.node), op, format_expr(&right.node))
        }
        Expr::UnaryOp(op, operand) => {
            let op_str = match op {
                UnaryOp::Not => "!",
                UnaryOp::Neg => "-",
            };
            format!("{}{}", op_str, format_expr(&operand.node))
        }
        Expr::Call(name, args) => {
            let arg_strs: Vec<String> = args.iter()
                .map(|a| format_expr(&a.node))
                .collect();
            format!("{}({})", name, arg_strs.join(", "))
        }
        Expr::Forall(vars, body) => {
            let var_strs: Vec<String> = vars.iter()
                .map(|v| format!("{}: {}", v.name, v.ty))
                .collect();
            format!("forall {}, {}", var_strs.join(", "), format_expr(&body.node))
        }
        Expr::Exists(vars, body) => {
            let var_strs: Vec<String> = vars.iter()
                .map(|v| format!("{}: {}", v.name, v.ty))
                .collect();
            format!("exists {}, {}", var_strs.join(", "), format_expr(&body.node))
        }
        Expr::IfThenElse(cond, then_br, else_br) => {
            format!("if {} then {} else {}",
                format_expr(&cond.node),
                format_expr(&then_br.node),
                format_expr(&else_br.node))
        }
        Expr::Paren(inner) => format!("({})", format_expr(&inner.node)),
        Expr::Index(arr, idx) => {
            format!("{}[{}]", format_expr(&arr.node), format_expr(&idx.node))
        }
    }
}

impl crate::GraphData for IntentGraph {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl crate::GraphData for VerificationFlow {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
