//! Render [intent-lang](https://github.com/popsicle-lab/intent-lang) programs as graphs.
//!
//! # Example
//!
//! ```
//! use intent_lang_syntax::parse;
//! use intent_lang_visualizer::{render_mermaid, VisKind};
//!
//! let program = parse(include_str!("../../../examples/basics/transfer.intent")).unwrap();
//! let mermaid = render_mermaid(&program, VisKind::GoalGraph);
//! assert!(mermaid.contains("graph LR"));
//! ```

pub mod coverage_matrix;
pub mod goal_graph;
pub mod graphviz;
pub mod intent_graph;
pub mod mermaid;
pub mod state_machine;

pub mod html_generator;

pub use coverage_matrix::{build_coverage_matrix, CoverageMatrix, CoverageStats, Dimension};
pub use goal_graph::{
    build_goal_graph, build_safety_network, Edge, EdgeType, GoalGraph, Node, NodeMetadata,
    NodeType,
};
pub use graphviz::{dot_to_svg, DotRenderable};
pub use intent_graph::{
    build_intent_graph, build_verification_flow, IntentEdge, IntentGraph, IntentNode,
    VerificationFlow,
};
pub use mermaid::MermaidRenderable;
pub use state_machine::{
    analyze_state_machine, build_state_machine, StateMachine, StateMachineReport, StateTransition,
};

use anyhow::{Context, Result};
use intent_lang_syntax::ast::Program;

/// Graph visualization kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VisKind {
    /// Goal → safety / intent / theorem dependency graph
    GoalGraph,
    /// Intent relationships and data flow
    IntentGraph,
    /// Safety rules and the types they constrain
    SafetyNetwork,
    /// Coverage dimension overview
    CoverageMatrix,
    /// Verification pipeline for intents
    VerificationFlow,
    /// Lifecycle state machine derived from status transitions
    StateMachine,
}

/// Output format for [`render`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Mermaid diagram wrapped in a Markdown fence (embeddable in docs)
    Mermaid,
    /// Raw Mermaid diagram body (no Markdown fence)
    MermaidRaw,
    /// Graphviz DOT
    Dot,
    /// JSON graph data
    Json,
    /// SVG via Graphviz (`dot` must be installed)
    Svg,
}

/// JSON serialization for graph structures.
pub trait GraphData {
    fn to_json(&self) -> Result<String>;
}

/// Parse source and render a visualization.
pub fn parse_and_render(source: &str, kind: VisKind, format: OutputFormat) -> Result<String> {
    let program = intent_lang_syntax::parse(source).context("failed to parse intent source")?;
    render(&program, kind, format)
}

/// Render Mermaid (Markdown-fenced) for a parsed program.
pub fn render_mermaid(program: &Program, kind: VisKind) -> String {
    render(program, kind, OutputFormat::Mermaid).expect("mermaid rendering is infallible")
}

/// Render raw Mermaid diagram text (no fence) for a parsed program.
pub fn render_mermaid_raw(program: &Program, kind: VisKind) -> String {
    render(program, kind, OutputFormat::MermaidRaw).expect("mermaid rendering is infallible")
}

/// Render a visualization for a parsed program.
pub fn render(program: &Program, kind: VisKind, format: OutputFormat) -> Result<String> {
    match kind {
        VisKind::GoalGraph => {
            let graph = goal_graph::build_goal_graph(program);
            let mut out = render_graph(&graph, format)?;
            if format == OutputFormat::Mermaid {
                if let Some(legend) = mermaid::goal_doc_legend(&graph) {
                    out.push_str(&legend);
                }
            }
            Ok(out)
        }
        VisKind::IntentGraph => {
            let graph = intent_graph::build_intent_graph(program);
            render_mermaid_or_json(&graph, format, "IntentGraph")
        }
        VisKind::SafetyNetwork => {
            let network = goal_graph::build_safety_network(program);
            render_graph(&network, format)
        }
        VisKind::CoverageMatrix => {
            let matrix = coverage_matrix::build_coverage_matrix(program);
            render_mermaid_or_json(&matrix, format, "CoverageMatrix")
        }
        VisKind::VerificationFlow => {
            let flow = intent_graph::build_verification_flow(program);
            render_mermaid_or_json(&flow, format, "VerificationFlow")
        }
        VisKind::StateMachine => {
            let sm = state_machine::build_state_machine(program);
            let mut out = render_mermaid_or_json(&sm, format, "StateMachine")?;
            if format == OutputFormat::Mermaid {
                if let Some(legend) = mermaid::state_doc_legend(&sm) {
                    out.push_str(&legend);
                }
            }
            Ok(out)
        }
    }
}

/// Render all standard graph kinds (excludes [`VisKind::VerificationFlow`]).
pub fn render_all(
    program: &Program,
    format: OutputFormat,
) -> Result<Vec<(VisKind, String)>> {
    let kinds = [
        VisKind::GoalGraph,
        VisKind::StateMachine,
        VisKind::SafetyNetwork,
        VisKind::CoverageMatrix,
    ];

    kinds
        .into_iter()
        .map(|kind| render(program, kind, format).map(|content| (kind, content)))
        .collect()
}

/// Strip the Markdown fence from a fenced Mermaid string.
pub fn unfence_mermaid(fenced: &str) -> String {
    fenced
        .trim()
        .strip_prefix("```mermaid\n")
        .and_then(|body| body.strip_suffix("```\n").or_else(|| body.strip_suffix("```")))
        .unwrap_or(fenced)
        .trim_end()
        .to_string()
}

fn render_graph<T>(graph: &T, format: OutputFormat) -> Result<String>
where
    T: GraphData + MermaidRenderable + DotRenderable,
{
    match format {
        OutputFormat::Mermaid => Ok(graph.to_mermaid()),
        OutputFormat::MermaidRaw => Ok(unfence_mermaid(&graph.to_mermaid())),
        OutputFormat::Dot => Ok(graph.to_dot()),
        OutputFormat::Json => graph.to_json(),
        OutputFormat::Svg => graphviz::dot_to_svg(&graph.to_dot()),
    }
}

fn render_mermaid_or_json<T>(value: &T, format: OutputFormat, name: &str) -> Result<String>
where
    T: GraphData + MermaidRenderable,
{
    match format {
        OutputFormat::Mermaid => Ok(value.to_mermaid()),
        OutputFormat::MermaidRaw => Ok(unfence_mermaid(&value.to_mermaid())),
        OutputFormat::Json => value.to_json(),
        OutputFormat::Dot | OutputFormat::Svg => {
            Err(anyhow::anyhow!("{name} only supports Mermaid and JSON formats"))
        }
    }
}
