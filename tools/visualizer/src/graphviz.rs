/// Graphviz DOT format renderer

use crate::goal_graph::{GoalGraph, NodeType, EdgeType};
use std::process::{Command, Stdio};
use std::io::Write;

pub fn render(graph: &impl DotRenderable) -> String {
    graph.to_dot()
}

pub trait DotRenderable {
    fn to_dot(&self) -> String;
}

impl DotRenderable for GoalGraph {
    fn to_dot(&self) -> String {
        let mut output = String::from("digraph IntentGraph {\n");
        output.push_str("    rankdir=TB;\n");
        output.push_str("    node [fontname=\"Arial\"];\n");
        output.push_str("    edge [fontname=\"Arial\"];\n\n");

        // Define node styles
        output.push_str("    // Node styles\n");
        for node in &self.nodes {
            let (shape, color, style) = match node.node_type {
                NodeType::Goal => ("box", "#e1f5ff", "filled,bold"),
                NodeType::Safety => ("diamond", "#fff3e0", "filled"),
                NodeType::Intent => ("ellipse", "#f3e5f5", "filled"),
                NodeType::Theorem => ("rectangle", "#e8f5e9", "filled"),
                NodeType::Axiom => ("parallelogram", "#fce4ec", "filled"),
            };

            let label = escape_dot(&node.label);

            output.push_str(&format!(
                "    \"{}\" [label=\"{}\", shape={}, fillcolor=\"{}\", style=\"{}\"];\n",
                node.id, label, shape, color, style
            ));
        }

        output.push_str("\n    // Edges\n");
        for edge in &self.edges {
            let (style, color) = match edge.edge_type {
                EdgeType::Realizes => ("solid", "black"),
                EdgeType::Validates => ("dashed", "green"),
                EdgeType::Enforces => ("bold", "red"),
                EdgeType::References => ("dotted", "gray"),
            };

            let label = edge.label.as_ref()
                .map(|l| format!(", label=\"{}\"", escape_dot(l)))
                .unwrap_or_default();

            output.push_str(&format!(
                "    \"{}\" -> \"{}\" [style={}, color={}{}];\n",
                edge.from, edge.to, style, color, label
            ));
        }

        output.push_str("}\n");
        output
    }
}

fn escape_dot(text: &str) -> String {
    text.replace("\"", "\\\"")
        .replace("\n", "\\n")
}

/// Convert DOT to SVG using graphviz
pub fn dot_to_svg(dot: &str) -> anyhow::Result<String> {
    let mut child = Command::new("dot")
        .arg("-Tsvg")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn 'dot' command. Is Graphviz installed? Error: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(dot.as_bytes())?;
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Graphviz error: {}", stderr));
    }

    Ok(String::from_utf8(output.stdout)?)
}
