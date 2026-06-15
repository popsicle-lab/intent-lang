use crate::coverage_matrix::CoverageMatrix;
use crate::goal_graph::{EdgeType, GoalGraph, NodeType};
use crate::intent_graph::{IntentGraph, VerificationFlow};

pub trait MermaidRenderable {
    fn to_mermaid(&self) -> String;
}

impl MermaidRenderable for GoalGraph {
    fn to_mermaid(&self) -> String {
        let mut output = String::from("```mermaid\ngraph TD\n");

        // Render nodes - simplified without HTML
        for node in &self.nodes {
            let style_class = match node.node_type {
                NodeType::Goal => "goalNode",
                NodeType::Safety => "safetyNode",
                NodeType::Intent => "intentNode",
                NodeType::Theorem => "theoremNode",
                NodeType::Axiom => "axiomNode",
            };

            // Simple shapes - () for safety instead of {} to avoid HTML issues
            let (shape_start, shape_end) = match node.node_type {
                NodeType::Goal => ("[", "]"),
                NodeType::Safety => ("(", ")"),
                NodeType::Intent => ("((", "))"),
                NodeType::Theorem => ("[[", "]]"),
                NodeType::Axiom => ("[/", "/]"),
            };

            output.push_str(&format!(
                "    {}{}\"{}\"{}:::{}\n",
                node.id.replace(" ", "_").replace("-", "_"),
                shape_start,
                node.label,
                shape_end,
                style_class
            ));
        }

        output.push('\n');

        // Render edges
        for edge in &self.edges {
            let arrow_style = match edge.edge_type {
                EdgeType::Realizes => "-->",
                EdgeType::Validates => "-.->",
                EdgeType::Enforces => "==>",
                EdgeType::References => "---",
            };

            let from = edge.from.replace(" ", "_").replace("-", "_");
            let to = edge.to.replace(" ", "_").replace("-", "_");

            match &edge.label {
                Some(label) => {
                    // Mermaid requires arrow and label adjacent: `-->|label| node`
                    output.push_str(&format!("    {from} {arrow_style}|{label}| {to}\n"));
                }
                None => {
                    output.push_str(&format!("    {from} {arrow_style} {to}\n"));
                }
            }
        }

        // Add styling
        output.push_str("\n    classDef goalNode fill:#e1f5ff,stroke:#01579b,stroke-width:2px\n");
        output.push_str("    classDef safetyNode fill:#fff3e0,stroke:#e65100,stroke-width:2px\n");
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
                node.id.replace(" ", "_").replace("-", "_"),
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
                    edge.from.replace(" ", "_").replace("-", "_"),
                    edge.to.replace(" ", "_").replace("-", "_")
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
