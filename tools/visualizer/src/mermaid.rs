use crate::coverage_matrix::CoverageMatrix;
use crate::goal_graph::{EdgeType, GoalGraph, NodeType};
use crate::intent_graph::{IntentGraph, VerificationFlow};
use crate::state_machine::{terminal_states, StateMachine};

pub trait MermaidRenderable {
    fn to_mermaid(&self) -> String;
}

/// Turn arbitrary label text into a valid Mermaid node identifier.
///
/// Mermaid IDs may only contain word characters; brackets, slashes, spaces and
/// punctuation (which free-text goal names like `[能力] 退款/退货...` contain)
/// break the graph. Collapse every non-word char to `_`.
fn sanitize_id(raw: &str) -> String {
    let mut out: String = raw
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    // A Mermaid id must not start with a digit or be empty.
    if out.is_empty() {
        out.push('_');
    } else if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        out.insert(0, '_');
    }
    out
}

impl MermaidRenderable for StateMachine {
    fn to_mermaid(&self) -> String {
        if self.state_enum.is_none() || self.transitions.is_empty() {
            return String::from(
                "```mermaid\nstateDiagram-v2\n    NoStateMachine: 未检测到状态型流转（模型无 status 枚举转换）\n```\n",
            );
        }

        let mut output = String::from("```mermaid\nstateDiagram-v2\n");

        // Creation edges.
        for s in &self.initial_states {
            output.push_str(&format!("    [*] --> {s}\n"));
        }

        // Transitions.
        for t in &self.transitions {
            output.push_str(&format!("    {} --> {}: {}\n", t.from, t.to, t.label));
        }

        // Terminal edges (targets that are never sources, excluding pure
        // creation-only states that already flow onward).
        for s in terminal_states(self) {
            if !self.initial_states.contains(&s) {
                output.push_str(&format!("    {s} --> [*]\n"));
            }
        }

        output.push_str("```\n");
        output
    }
}

impl MermaidRenderable for GoalGraph {
    fn to_mermaid(&self) -> String {
        // Left-right layout reads better once nodes are boxed into clusters.
        let mut output = String::from("```mermaid\ngraph LR\n");

        let node_by_id: std::collections::HashMap<&str, &crate::goal_graph::Node> =
            self.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        let render_node = |out: &mut String, node: &crate::goal_graph::Node, indent: &str| {
            use crate::goal_graph::GoalKind;
            // Goal color is driven by capability vs guardrail; realizers by type.
            let style_class = match node.node_type {
                NodeType::Goal => match node.goal_kind {
                    Some(GoalKind::Capability) => "capabilityNode",
                    Some(GoalKind::Guardrail) => "guardrailNode",
                    None => "goalNode",
                },
                NodeType::Safety => "safetyNode",
                NodeType::Intent => "intentNode",
                NodeType::Theorem => "theoremNode",
                NodeType::Axiom => "axiomNode",
            };
            let (shape_start, shape_end) = match node.node_type {
                NodeType::Goal => ("[", "]"),
                NodeType::Safety => ("(", ")"),
                NodeType::Intent => ("((", "))"),
                NodeType::Theorem => ("[[", "]]"),
                NodeType::Axiom => ("[/", "/]"),
            };
            out.push_str(&format!(
                "{indent}{}{}\"{}\"{}:::{}\n",
                sanitize_id(&node.id),
                shape_start,
                node.label,
                shape_end,
                style_class
            ));
        };

        if self.clusters.is_empty() {
            // Flat mode: no goal carried a theme group.
            for node in &self.nodes {
                render_node(&mut output, node, "    ");
            }
        } else {
            for (i, cluster) in self.clusters.iter().enumerate() {
                output.push_str(&format!(
                    "    subgraph cluster{i}[\"{}\"]\n",
                    cluster.title
                ));
                for id in &cluster.node_ids {
                    if let Some(node) = node_by_id.get(id.as_str()) {
                        render_node(&mut output, node, "        ");
                    }
                }
                output.push_str("    end\n");
            }
        }

        output.push('\n');

        // Edges — no labels (the arrow already means "realized_by"; 37 identical
        // labels are pure noise).
        for edge in &self.edges {
            let arrow_style = match edge.edge_type {
                EdgeType::Realizes => "-->",
                EdgeType::Validates => "-.->",
                EdgeType::Enforces => "==>",
                EdgeType::References => "---",
            };
            output.push_str(&format!(
                "    {} {arrow_style} {}\n",
                sanitize_id(&edge.from),
                sanitize_id(&edge.to)
            ));
        }

        // Styling — capability (positive/green) vs guardrail (amber) split.
        output.push_str("\n    classDef capabilityNode fill:#e8f5e9,stroke:#1b5e20,stroke-width:2px\n");
        output.push_str("    classDef guardrailNode fill:#fff3e0,stroke:#e65100,stroke-width:2px\n");
        output.push_str("    classDef goalNode fill:#e1f5ff,stroke:#01579b,stroke-width:2px\n");
        output.push_str("    classDef safetyNode fill:#fbe9e7,stroke:#bf360c,stroke-width:1px\n");
        output.push_str("    classDef intentNode fill:#f3e5f5,stroke:#4a148c,stroke-width:2px\n");
        output.push_str("    classDef theoremNode fill:#e8f5e9,stroke:#1b5e20,stroke-width:2px\n");
        output.push_str("    classDef axiomNode fill:#fce4ec,stroke:#880e4f,stroke-width:2px\n");

        output.push_str("```\n");
        output
    }
}

impl MermaidRenderable for IntentGraph {
    fn to_mermaid(&self) -> String {
        let mut output = String::from("```mermaid\ngraph TD\n");

        // Render nodes - super simple
        for node in &self.nodes {
            let style_class = if node.annotations.contains(&"tobe".to_string()) {
                "tobeNode"
            } else if node.annotations.contains(&"asis".to_string()) {
                "asisNode"
            } else {
                "intentNode"
            };

            output.push_str(&format!(
                "    {}[\"{}\"]:::{}\n",
                sanitize_id(&node.id),
                node.name,
                style_class
            ));
        }

        // Render edges - no labels
        if !self.edges.is_empty() {
            output.push_str("\n");
            for edge in &self.edges {
                output.push_str(&format!(
                    "    {} -.-> {}\n",
                    sanitize_id(&edge.from),
                    sanitize_id(&edge.to)
                ));
            }
        }

        // Add styling
        output.push_str("\n    classDef tobeNode fill:#e1f5ff,stroke:#01579b,stroke-width:2px\n");
        output.push_str("    classDef asisNode fill:#fff8e1,stroke:#f57c00,stroke-width:2px\n");
        output.push_str("    classDef intentNode fill:#f3e5f5,stroke:#4a148c,stroke-width:2px\n");

        output.push_str("```\n");
        output
    }
}

impl MermaidRenderable for CoverageMatrix {
    fn to_mermaid(&self) -> String {
        let mut output = String::from("```mermaid\ngraph LR\n");

        // Simple - no emoji or HTML
        output.push_str(&format!("    Coverage[\"{}\"]\n", self.name));

        for (idx, dim) in self.dimensions.iter().enumerate() {
            let dim_id = format!("dim{}", idx);
            output.push_str(&format!(
                "    {}[\"{}: {} values\"]\n",
                dim_id,
                dim.name,
                dim.values.len()
            ));
            output.push_str(&format!("    Coverage --> {}\n", dim_id));
        }

        if let Some(stats) = &self.stats {
            output.push_str(&format!(
                "\n    Stats[\"Total: {} | Covered: {} | Missing: {}\"]\n",
                stats.total_combinations,
                stats.covered_combinations,
                stats.missing_combinations
            ));
            output.push_str("    Coverage --> Stats\n");
        }

        output.push_str("```\n");
        output
    }
}

impl MermaidRenderable for VerificationFlow {
    fn to_mermaid(&self) -> String {
        let mut output = String::from("```mermaid\ngraph TD\n");

        // Simple verification flow
        output.push_str(&format!("    Intent[\"Intent: {}\"]\n", self.intent_name));
        output.push_str("    WP[\"Weakest Precondition\"]\n");
        output.push_str("    VC[\"Verification Conditions\"]\n");
        output.push_str("    Z3[\"Z3 Solver\"]\n");

        output.push_str("\n    Intent --> WP\n");
        output.push_str("    WP --> VC\n");
        output.push_str("    VC --> Z3\n");

        output.push_str("```\n");
        output
    }
}

/// Markdown legend table mapping node names to their `@doc` description.
/// Returns `None` when no node carries a description.
pub fn goal_doc_legend(graph: &GoalGraph) -> Option<String> {
    let rows: Vec<(&str, &str)> = graph
        .nodes
        .iter()
        .filter_map(|n| n.metadata.doc.as_deref().map(|d| (n.label.as_str(), d)))
        .collect();
    doc_legend_table(&rows)
}

/// Markdown legend for the operations that drive state transitions.
pub fn state_doc_legend(sm: &StateMachine) -> Option<String> {
    let rows: Vec<(&str, &str)> = sm
        .intent_docs
        .iter()
        .map(|(name, doc)| (name.as_str(), doc.as_str()))
        .collect();
    doc_legend_table(&rows)
}

fn doc_legend_table(rows: &[(&str, &str)]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    let mut out = String::from("\n**操作说明**\n\n| 名称 | 说明 |\n| --- | --- |\n");
    for (name, doc) in rows {
        // Escape pipes so the description can't break the table.
        let doc = doc.replace('|', "\\|");
        out.push_str(&format!("| `{name}` | {doc} |\n"));
    }
    Some(out)
}
