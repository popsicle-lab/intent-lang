use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use intent_visualizer::{
    html_generator, render, render_all, OutputFormat as LibOutputFormat, VisKind,
};
use intent_syntax::parser::Parser as IntentParser;

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

#[derive(Clone, Copy, ValueEnum, Debug)]
enum VisType {
    GoalGraph,
    IntentGraph,
    SafetyNetwork,
    CoverageMatrix,
    VerificationFlow,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Mermaid,
    MermaidRaw,
    Dot,
    Json,
    Svg,
}

impl From<VisType> for VisKind {
    fn from(value: VisType) -> Self {
        match value {
            VisType::GoalGraph => VisKind::GoalGraph,
            VisType::IntentGraph => VisKind::IntentGraph,
            VisType::SafetyNetwork => VisKind::SafetyNetwork,
            VisType::CoverageMatrix => VisKind::CoverageMatrix,
            VisType::VerificationFlow => VisKind::VerificationFlow,
        }
    }
}

impl From<OutputFormat> for LibOutputFormat {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Mermaid => LibOutputFormat::Mermaid,
            OutputFormat::MermaidRaw => LibOutputFormat::MermaidRaw,
            OutputFormat::Dot => LibOutputFormat::Dot,
            OutputFormat::Json => LibOutputFormat::Json,
            OutputFormat::Svg => LibOutputFormat::Svg,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let source = std::fs::read_to_string(&cli.input)
        .with_context(|| format!("Failed to read file: {:?}", cli.input))?;

    let mut parser = IntentParser::new(&source)?;
    let program = parser
        .parse_program()
        .map_err(|e| anyhow::anyhow!("Parse error: {:?}", e))?;

    if cli.interactive {
        let html = html_generator::generate_interactive_html(&program, &source)?;
        output_result(&cli, html)?;
    } else if cli.all {
        generate_all_visualizations(&cli, &program)?;
    } else {
        let output = render(&program, cli.r#type.into(), cli.format.into())?;
        output_result(&cli, output)?;
    }

    Ok(())
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

fn generate_all_visualizations(
    cli: &Cli,
    program: &intent_syntax::ast::Program,
) -> Result<()> {
    std::fs::create_dir_all(&cli.output_dir)?;

    for (vis_kind, output) in render_all(program, cli.format.into())? {
        let filename = format!("{:?}.{}", vis_kind, extension_for_format(&cli.format));
        let path = cli.output_dir.join(filename.to_lowercase());
        std::fs::write(&path, output)?;
        eprintln!("✓ Generated {:?}", path);
    }

    let html = html_generator::generate_index_html(&cli.output_dir)?;
    let index_path = cli.output_dir.join("index.html");
    std::fs::write(&index_path, html)?;
    eprintln!("✓ Generated {:?}", index_path);

    Ok(())
}

fn extension_for_format(format: &OutputFormat) -> &str {
    match format {
        OutputFormat::Mermaid | OutputFormat::MermaidRaw => "mmd",
        OutputFormat::Dot => "dot",
        OutputFormat::Json => "json",
        OutputFormat::Svg => "svg",
    }
}
