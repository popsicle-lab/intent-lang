use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

mod goal_graph;
mod intent_graph;
mod coverage_matrix;
mod mermaid;
mod graphviz;
mod html_generator;

use intent_syntax::parser::Parser as IntentParser;
use mermaid::MermaidRenderable;
use graphviz::DotRenderable;

#[derive(Parser)]
#[command(name = "intent-visualizer")]
#[command(about = "Visualize intent-lang business intents", long_about = None)]
struct Cli {
    /// Input .intent file
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Visualization type
    #[arg(short, long, value_enum, default_value = "goal-graph")]
    r#type: VisType,

    /// Output format
    #[arg(short, long, value_enum, default_value = "mermaid")]
    format: OutputFormat,

    /// Output file (stdout if not specified)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Generate all visualization types
    #[arg(long)]
    all: bool,

    /// Output directory for --all mode
    #[arg(long, default_value = "./viz")]
    output_dir: PathBuf,

    /// Generate interactive HTML
    #[arg(long)]
    interactive: bool,
}

#[derive(Clone, ValueEnum, Debug)]
enum VisType {
    /// Goal → Safety → Intent → Theorem dependency graph
    GoalGraph,
    /// Intent relationship and data flow
    IntentGraph,
    /// Safety rule coverage network
    SafetyNetwork,
    /// Coverage dimension heatmap
    CoverageMatrix,
    /// Verification flow for a single intent
    VerificationFlow,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    /// Mermaid diagram (Markdown-embeddable)
    Mermaid,
    /// Graphviz DOT format
    Dot,
    /// D3.js JSON data
    Json,
    /// Standalone SVG
    Svg,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Read and parse the intent file
    let source = std::fs::read_to_string(&cli.input)
        .with_context(|| format!("Failed to read file: {:?}", cli.input))?;

    let mut parser = IntentParser::new(&source)?;
    let program = parser.parse_program()
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    if cli.interactive {
        // Generate interactive HTML with all visualizations
        let html = html_generator::generate_interactive_html(&program, &source)?;
        output_result(&cli, html)?;
    } else if cli.all {
        // Generate all visualization types
        generate_all_visualizations(&cli, &program)?;
    } else {
        // Generate single visualization
        let output = generate_visualization(&cli.r#type, &cli.format, &program)?;
        output_result(&cli, output)?;
    }

    Ok(())
}

fn generate_visualization(
    vis_type: &VisType,
    format: &OutputFormat,
    program: &intent_syntax::ast::Program,
) -> Result<String> {
    match vis_type {
        VisType::GoalGraph => {
            let graph = goal_graph::build_goal_graph(program);
            match format {
                OutputFormat::Mermaid => Ok(graph.to_mermaid()),
                OutputFormat::Dot => Ok(graph.to_dot()),
                OutputFormat::Json => Ok(graph.to_json()?),
                OutputFormat::Svg => {
                    let dot = graph.to_dot();
                    graphviz::dot_to_svg(&dot)
                }
            }
        }
        VisType::IntentGraph => {
            let graph = intent_graph::build_intent_graph(program);
            match format {
                OutputFormat::Mermaid => Ok(graph.to_mermaid()),
                OutputFormat::Json => Ok(graph.to_json()?),
                _ => Err(anyhow::anyhow!("IntentGraph only supports Mermaid and JSON formats")),
            }
        }
        VisType::SafetyNetwork => {
            let network = goal_graph::build_safety_network(program);
            match format {
                OutputFormat::Mermaid => Ok(network.to_mermaid()),
                OutputFormat::Dot => Ok(network.to_dot()),
                OutputFormat::Json => Ok(network.to_json()?),
                OutputFormat::Svg => {
                    let dot = network.to_dot();
                    graphviz::dot_to_svg(&dot)
                }
            }
        }
        VisType::CoverageMatrix => {
            let matrix = coverage_matrix::build_coverage_matrix(program);
            match format {
                OutputFormat::Mermaid => Ok(matrix.to_mermaid()),
                OutputFormat::Json => Ok(matrix.to_json()?),
                _ => Err(anyhow::anyhow!("CoverageMatrix only supports Mermaid and JSON formats")),
            }
        }
        VisType::VerificationFlow => {
            let flow = intent_graph::build_verification_flow(program);
            match format {
                OutputFormat::Mermaid => Ok(flow.to_mermaid()),
                OutputFormat::Json => Ok(flow.to_json()?),
                _ => Err(anyhow::anyhow!("VerificationFlow only supports Mermaid and JSON formats")),
            }
        }
    }
}

fn format_output<T>(graph: &T, format: &OutputFormat) -> Result<String>
where
    T: GraphData + MermaidRenderable + DotRenderable,
{
    match format {
        OutputFormat::Mermaid => Ok(graph.to_mermaid()),
        OutputFormat::Dot => Ok(graph.to_dot()),
        OutputFormat::Json => Ok(graph.to_json()?),
        OutputFormat::Svg => {
            let dot = graph.to_dot();
            graphviz::dot_to_svg(&dot)
        }
    }
}

fn output_result(cli: &Cli, content: String) -> Result<()> {
    match &cli.output {
        Some(path) => {
            std::fs::write(path, content)
                .with_context(|| format!("Failed to write to {:?}", path))?;
            eprintln!("✓ Written to {:?}", path);
        }
        None => {
            println!("{}", content);
        }
    }
    Ok(())
}

fn generate_all_visualizations(cli: &Cli, program: &intent_syntax::ast::Program) -> Result<()> {
    std::fs::create_dir_all(&cli.output_dir)?;

    let types = vec![
        VisType::GoalGraph,
        VisType::IntentGraph,
        VisType::SafetyNetwork,
        VisType::CoverageMatrix,
    ];

    for vis_type in types {
        let output = generate_visualization(&vis_type, &cli.format, program)?;
        let filename = format!("{:?}.{}", vis_type, extension_for_format(&cli.format));
        let path = cli.output_dir.join(filename.to_lowercase());
        std::fs::write(&path, output)?;
        eprintln!("✓ Generated {:?}", path);
    }

    // Generate index.html
    let html = html_generator::generate_index_html(&cli.output_dir)?;
    let index_path = cli.output_dir.join("index.html");
    std::fs::write(&index_path, html)?;
    eprintln!("✓ Generated {:?}", index_path);

    Ok(())
}

fn extension_for_format(format: &OutputFormat) -> &str {
    match format {
        OutputFormat::Mermaid => "mmd",
        OutputFormat::Dot => "dot",
        OutputFormat::Json => "json",
        OutputFormat::Svg => "svg",
    }
}

// Trait for graph data structures
trait GraphData {
    fn to_json(&self) -> Result<String>;
}
