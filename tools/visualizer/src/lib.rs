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
pub mod flowchart;
pub mod goal_graph;
pub mod graphviz;
pub mod intent_graph;
pub mod mermaid;
pub mod model;
pub mod source_view;
pub mod state_machine;

pub mod html_generator;

pub use coverage_matrix::{
    build_all_coverage_matrices, build_coverage_matrix, CoverageMatrix, CoverageStats, Dimension,
};
pub use flowchart::{build_flowchart, Flowchart};
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
pub use model::{build_doc_model, DocModel};
pub use source_view::render_source_html;
pub use state_machine::{
    analyze_state_machine, build_state_machine, build_state_machine_for, lifecycle_enums,
    lifecycle_state_machines, StateMachine, StateMachineReport, StateTransition,
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
    /// Business process flowchart (operation boxes + decision diamonds)
    Flowchart,
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
                if let Some(note) = mermaid::state_conflict_note(&sm) {
                    out.push_str(&note);
                }
                if let Some(legend) = mermaid::state_doc_legend(&sm) {
                    out.push_str(&legend);
                }
            }
            Ok(out)
        }
        VisKind::Flowchart => {
            let fc = flowchart::build_flowchart(program);
            let mut out = render_mermaid_or_json(&fc, format, "Flowchart")?;
            if format == OutputFormat::Mermaid {
                // Reuse the state-machine's conflict + operation legends so the
                // flowchart carries the same annotations beneath the diagram.
                let sm = state_machine::build_state_machine(program);
                if let Some(note) = mermaid::state_conflict_note(&sm) {
                    out.push_str(&note);
                }
                if let Some(legend) = mermaid::state_doc_legend(&sm) {
                    out.push_str(&legend);
                }
            }
            Ok(out)
        }
    }
}

/// Render the state machine of one specific `@lifecycle` enum. `--all` uses
/// this to emit a diagram per declared lifecycle instead of only the first —
/// a file with two lifecycles otherwise showed just one of them.
pub fn render_state_machine_of(
    program: &Program,
    state_enum: &str,
    format: OutputFormat,
) -> Result<String> {
    let sm = state_machine::build_state_machine_for(program, state_enum);
    let mut out = render_mermaid_or_json(&sm, format, "StateMachine")?;
    if format == OutputFormat::Mermaid {
        if let Some(note) = mermaid::state_conflict_note(&sm) {
            out.push_str(&note);
        }
        if let Some(legend) = mermaid::state_doc_legend(&sm) {
            out.push_str(&legend);
        }
    }
    Ok(out)
}

/// Render the standard graph kinds embedded in the interactive page
/// (excludes [`VisKind::VerificationFlow`] and [`VisKind::SafetyNetwork`] —
/// the latter's two-bipartite-graph view was retired in favor of the
/// goal-grouped safety-rule table on the interactive page; it remains
/// available via `--type safety-network`).
pub fn render_all(
    program: &Program,
    format: OutputFormat,
) -> Result<Vec<(VisKind, String)>> {
    let kinds = [
        VisKind::GoalGraph,
        VisKind::StateMachine,
        VisKind::Flowchart,
        VisKind::CoverageMatrix,
    ];

    kinds
        .into_iter()
        .map(|kind| render(program, kind, format).map(|content| (kind, content)))
        .collect()
}

/// Escape text for use as HTML element content.
pub(crate) fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape text for use inside a double-quoted HTML attribute value.
pub(crate) fn html_escape_attr(text: &str) -> String {
    html_escape(text).replace('"', "&quot;")
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
