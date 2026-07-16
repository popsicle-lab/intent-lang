//! Business process **flowchart** (`flowchart TD`): operation boxes wired
//! through state nodes, where a state with two or more outgoing operations
//! becomes a **decision diamond**.
//!
//! Unlike the `state-machine` view (states are nodes, operations are edge
//! labels — a lifecycle diagram), this view is closer to a traditional
//! business flowchart: each operation is a process box, branch points are
//! diamonds, and start/terminal are capsules.
//!
//! It is derived from the same faithful transition data as the state machine
//! (`require` source-state → `ensure` target-state), so it never invents flow
//! that isn't in the `.intent`.

use crate::mermaid::{sanitize_id, MermaidRenderable};
use crate::state_machine::{build_state_machine, terminal_states};
use crate::GraphData;
use intent_lang_syntax::ast::Program;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Flowchart {
    pub state_enum: Option<String>,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    /// `(operation, @doc)` for the legend beneath the diagram.
    pub intent_docs: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
pub struct FlowNode {
    pub id: String,
    pub label: String,
    pub kind: FlowNodeKind,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum FlowNodeKind {
    Start,
    /// A process box; `conflict` marks a structurally self-contradictory op.
    Operation { conflict: bool },
    /// A state with ≥2 outgoing operations → rendered as a decision diamond.
    Decision,
    /// A state with a single outgoing operation → pass-through node.
    StateLinear,
    /// A terminal state (no outgoing operation).
    Terminal,
}

#[derive(Debug, Serialize)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
}

fn op_id(name: &str) -> String {
    format!("op_{}", sanitize_id(name))
}

fn state_id(name: &str) -> String {
    format!("s_{}", sanitize_id(name))
}

pub fn build_flowchart(program: &Program) -> Flowchart {
    let sm = build_state_machine(program);
    let Some(state_enum) = sm.state_enum.clone() else {
        return Flowchart {
            state_enum: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            intent_docs: Vec::new(),
        };
    };

    // (source-state | None for creation, operation, target-state)
    let mut triples: Vec<(Option<String>, String, String)> = Vec::new();
    for t in &sm.creation {
        for op in t.label.split('/') {
            triples.push((None, op.trim().to_string(), t.to.clone()));
        }
    }
    for t in &sm.transitions {
        for op in t.label.split('/') {
            triples.push((Some(t.from.clone()), op.trim().to_string(), t.to.clone()));
        }
    }

    // Distinct outgoing operations per state → decides diamond vs pass-through.
    let mut out_ops: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (from, op, _to) in &triples {
        if let Some(f) = from {
            out_ops.entry(f.clone()).or_default().insert(op.clone());
        }
    }

    let terminals: BTreeSet<String> = terminal_states(&sm).into_iter().collect();
    let conflict_ops: BTreeSet<String> = sm.conflicts.iter().map(|c| c.intent.clone()).collect();

    let mut states: BTreeSet<String> = BTreeSet::new();
    let mut ops: BTreeSet<String> = BTreeSet::new();
    for (from, op, to) in &triples {
        if let Some(f) = from {
            states.insert(f.clone());
        }
        states.insert(to.clone());
        ops.insert(op.clone());
    }

    let mut nodes = Vec::new();
    nodes.push(FlowNode {
        id: "start".to_string(),
        label: "开始".to_string(),
        kind: FlowNodeKind::Start,
    });
    for op in &ops {
        nodes.push(FlowNode {
            id: op_id(op),
            label: op.clone(),
            kind: FlowNodeKind::Operation {
                conflict: conflict_ops.contains(op),
            },
        });
    }
    for s in &states {
        let kind = if terminals.contains(s) {
            FlowNodeKind::Terminal
        } else if out_ops.get(s).map(|o| o.len()).unwrap_or(0) >= 2 {
            FlowNodeKind::Decision
        } else {
            FlowNodeKind::StateLinear
        };
        nodes.push(FlowNode {
            id: state_id(s),
            label: s.clone(),
            kind,
        });
    }

    let mut edgeset: BTreeSet<(String, String)> = BTreeSet::new();
    for (from, op, to) in &triples {
        let oid = op_id(op);
        match from {
            None => {
                edgeset.insert(("start".to_string(), oid.clone()));
            }
            Some(f) => {
                edgeset.insert((state_id(f), oid.clone()));
            }
        }
        edgeset.insert((oid, state_id(to)));
    }
    let edges = edgeset
        .into_iter()
        .map(|(from, to)| FlowEdge { from, to })
        .collect();

    Flowchart {
        state_enum: Some(state_enum),
        nodes,
        edges,
        intent_docs: sm.intent_docs.clone(),
    }
}

impl MermaidRenderable for Flowchart {
    fn to_mermaid(&self) -> String {
        if self.state_enum.is_none() || self.edges.is_empty() {
            return String::from(
                "```mermaid\nflowchart TD\n    NoFlow[\"未检测到状态型流转（模型无 status 枚举转换）\"]\n```\n",
            );
        }

        let mut out = String::from("```mermaid\nflowchart TD\n");

        for n in &self.nodes {
            let line = match &n.kind {
                FlowNodeKind::Start => format!("    {}([\"{}\"]):::startNode\n", n.id, n.label),
                FlowNodeKind::Operation { conflict } => {
                    let label = if *conflict {
                        format!("{} ⚠", n.label)
                    } else {
                        n.label.clone()
                    };
                    let class = if *conflict { "conflictOp" } else { "opNode" };
                    format!("    {}[\"{}\"]:::{}\n", n.id, label, class)
                }
                FlowNodeKind::Decision => {
                    format!("    {}{{\"{}\"}}:::decisionNode\n", n.id, n.label)
                }
                FlowNodeKind::StateLinear => {
                    format!("    {}(\"{}\"):::stateNode\n", n.id, n.label)
                }
                FlowNodeKind::Terminal => {
                    format!("    {}([\"{}\"]):::terminalNode\n", n.id, n.label)
                }
            };
            out.push_str(&line);
        }

        out.push('\n');
        for e in &self.edges {
            out.push_str(&format!("    {} --> {}\n", e.from, e.to));
        }

        out.push_str("\n    classDef startNode fill:#e3f2fd,stroke:#0d47a1,stroke-width:2px\n");
        out.push_str("    classDef opNode fill:#f3e5f5,stroke:#4a148c,stroke-width:1px\n");
        out.push_str("    classDef decisionNode fill:#fff8e1,stroke:#f57f17,stroke-width:2px\n");
        out.push_str("    classDef stateNode fill:#e8f5e9,stroke:#1b5e20,stroke-width:1px\n");
        out.push_str("    classDef terminalNode fill:#eceff1,stroke:#37474f,stroke-width:2px\n");
        out.push_str("    classDef conflictOp fill:#fdecea,stroke:#c62828,stroke-width:2px\n");

        out.push_str("```\n");
        out
    }
}

impl GraphData for Flowchart {
    fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
