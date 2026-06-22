/// Goal dependency graph builder
///
/// Builds a graph showing:
/// - Goals at the top level
/// - Safety rules, Intents, and Theorems that realize each goal
/// - Cross-references between declarations

use intent_lang_syntax::ast::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct GoalGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
    pub metadata: NodeMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum NodeType {
    Goal,
    Safety,
    Intent,
    Theorem,
    Axiom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub rationale: Option<String>,
    pub stakeholders: Vec<String>,
    pub annotations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EdgeType {
    Realizes,      // Goal → Safety/Intent/Theorem
    Validates,     // Theorem → Intent
    Enforces,      // Safety → Intent
    References,    // Generic reference
}

pub fn build_goal_graph(program: &Program) -> GoalGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    // Build index of all declarations by name
    let mut decl_index: HashMap<String, &Declaration> = HashMap::new();

    for decl in &program.declarations {
        match &decl.node {
            Declaration::Goal(g) => {
                decl_index.insert(g.name.clone(), &decl.node);
            }
            Declaration::Safety(s) => {
                decl_index.insert(s.name.clone(), &decl.node);
            }
            Declaration::Intent(i) => {
                decl_index.insert(i.name.clone(), &decl.node);
            }
            Declaration::Theorem(t) => {
                decl_index.insert(t.name.clone(), &decl.node);
            }
            Declaration::Axiom(a) => {
                decl_index.insert(a.name.clone(), &decl.node);
            }
            _ => {}
        }
    }

    // Process each declaration
    for decl in &program.declarations {
        match &decl.node {
            Declaration::Goal(goal) => {
                // Add goal node
                nodes.push(Node {
                    id: goal.name.clone(),
                    label: goal.name.clone(),
                    node_type: NodeType::Goal,
                    metadata: NodeMetadata {
                        rationale: goal.rationale.clone(),
                        stakeholders: goal.stakeholder.clone(),
                        annotations: vec![],
                    },
                });

                // Add edges to realized_by declarations
                for realized in &goal.realized_by {
                    if decl_index.contains_key(realized) {
                        edges.push(Edge {
                            from: goal.name.clone(),
                            to: realized.clone(),
                            edge_type: EdgeType::Realizes,
                            label: Some("realized_by".to_string()),
                        });
                    }
                }
            }

            Declaration::Safety(safety) => {
                nodes.push(Node {
                    id: safety.name.clone(),
                    label: safety.name.clone(),
                    node_type: NodeType::Safety,
                    metadata: NodeMetadata {
                        rationale: None,
                        stakeholders: vec![],
                        annotations: vec![],
                    },
                });
            }

            Declaration::Intent(intent) => {
                let annotations: Vec<String> = intent.annotations
                    .iter()
                    .map(|a| a.name.clone())
                    .collect();

                nodes.push(Node {
                    id: intent.name.clone(),
                    label: intent.name.clone(),
                    node_type: NodeType::Intent,
                    metadata: NodeMetadata {
                        rationale: None,
                        stakeholders: vec![],
                        annotations,
                    },
                });
            }

            Declaration::Theorem(theorem) => {
                nodes.push(Node {
                    id: theorem.name.clone(),
                    label: theorem.name.clone(),
                    node_type: NodeType::Theorem,
                    metadata: NodeMetadata {
                        rationale: None,
                        stakeholders: vec![],
                        annotations: vec![],
                    },
                });

                // Extract intent references from theorem body
                let referenced_intents = extract_intent_references(&theorem.body);
                for intent_name in referenced_intents {
                    if decl_index.contains_key(&intent_name) {
                        edges.push(Edge {
                            from: theorem.name.clone(),
                            to: intent_name,
                            edge_type: EdgeType::Validates,
                            label: Some("validates".to_string()),
                        });
                    }
                }
            }

            Declaration::Axiom(axiom) => {
                nodes.push(Node {
                    id: axiom.name.clone(),
                    label: axiom.name.clone(),
                    node_type: NodeType::Axiom,
                    metadata: NodeMetadata {
                        rationale: None,
                        stakeholders: vec![],
                        annotations: vec![],
                    },
                });
            }

            _ => {}
        }
    }

    GoalGraph { nodes, edges }
}

pub fn build_safety_network(program: &Program) -> GoalGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut type_nodes: HashSet<String> = HashSet::new();

    // Collect all type definitions
    for decl in &program.declarations {
        if let Declaration::Type(type_decl) = &decl.node {
            type_nodes.insert(type_decl.name.clone());

            nodes.push(Node {
                id: format!("type_{}", type_decl.name),
                label: type_decl.name.clone(),
                node_type: NodeType::Safety,
                metadata: NodeMetadata {
                    rationale: Some("Domain type".to_string()),
                    stakeholders: vec![],
                    annotations: vec![],
                },
            });
        }
    }

    // Process safety rules
    for decl in &program.declarations {
        if let Declaration::Safety(safety) = &decl.node {
            nodes.push(Node {
                id: safety.name.clone(),
                label: safety.name.clone(),
                node_type: NodeType::Safety,
                metadata: NodeMetadata {
                    rationale: Some(format!("{} rules", safety.invariants.len())),
                    stakeholders: vec![],
                    annotations: vec![],
                },
            });

            // Link safety rules to the types they constrain
            for param in &safety.params {
                if let TypeExpr::Named(type_name) = &param.ty {
                    if type_nodes.contains(type_name) {
                        edges.push(Edge {
                            from: safety.name.clone(),
                            to: format!("type_{}", type_name),
                            edge_type: EdgeType::Enforces,
                            label: Some(format!("param: {}", param.name)),
                        });
                    }
                }
            }
        }
    }

    GoalGraph { nodes, edges }
}

/// Extract intent names from expressions (simplified)
fn extract_intent_references(expr: &Spanned<Expr>) -> Vec<String> {
    let mut refs = Vec::new();
    extract_intent_refs_recursive(&expr.node, &mut refs);
    refs
}

fn extract_intent_refs_recursive(expr: &Expr, refs: &mut Vec<String>) {
    match expr {
        Expr::Call(name, args) => {
            // Assume PascalCase names are intent references
            if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                refs.push(name.clone());
            }
            for arg in args {
                extract_intent_refs_recursive(&arg.node, refs);
            }
        }
        Expr::BinOp(left, _, right) => {
            extract_intent_refs_recursive(&left.node, refs);
            extract_intent_refs_recursive(&right.node, refs);
        }
        Expr::UnaryOp(_, operand) => {
            extract_intent_refs_recursive(&operand.node, refs);
        }
        Expr::IfThenElse(cond, then_branch, else_branch) => {
            extract_intent_refs_recursive(&cond.node, refs);
            extract_intent_refs_recursive(&then_branch.node, refs);
            extract_intent_refs_recursive(&else_branch.node, refs);
        }
        Expr::Forall(_, body) | Expr::Exists(_, body) => {
            extract_intent_refs_recursive(&body.node, refs);
        }
        Expr::Prime(inner) => {
            extract_intent_refs_recursive(&inner.node, refs);
        }
        Expr::FieldAccess(obj, _) => {
            extract_intent_refs_recursive(&obj.node, refs);
        }
        Expr::Index(arr, idx) => {
            extract_intent_refs_recursive(&arr.node, refs);
            extract_intent_refs_recursive(&idx.node, refs);
        }
        Expr::Paren(inner) => {
            extract_intent_refs_recursive(&inner.node, refs);
        }
        _ => {}
    }
}

impl crate::GraphData for GoalGraph {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
