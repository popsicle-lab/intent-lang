use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use intent_lang_visualizer::{
    analyze_state_machine, build_state_machine, html_generator, render, render_all,
    OutputFormat as LibOutputFormat, VisKind,
};
use intent_lang_syntax::parser::Parser as IntentParser;

#[derive(Parser)]
#[command(name = "intent-lang-visualizer")]
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

    /// Run structural liveness checks on the derived state machine
    /// (reachability / dead states / trapped cycles). Exits non-zero on issues.
    #[arg(long)]
    check_states: bool,
}

#[derive(Clone, Copy, ValueEnum, Debug)]
enum VisType {
    GoalGraph,
    IntentGraph,
    SafetyNetwork,
    CoverageMatrix,
    VerificationFlow,
    StateMachine,
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
            VisType::StateMachine => VisKind::StateMachine,
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

    if cli.check_states {
        return check_states(&program);
    }

    if cli.interactive {
        let html = html_generator::generate_interactive_html(&program, &source)?;
        output_result(&cli, html)?;
    } else if cli.all {
        generate_all_visualizations(&cli, &program, &source)?;
    } else {
        let output = render(&program, cli.r#type.into(), cli.format.into())?;
        output_result(&cli, output)?;
    }

    Ok(())
}

fn check_states(program: &intent_lang_syntax::ast::Program) -> Result<()> {
    let sm = build_state_machine(program);
    let Some(state_enum) = &sm.state_enum else {
        println!("ℹ️  No dominant status enum detected — skipping state-machine liveness checks.");
        return Ok(());
    };

    let report = analyze_state_machine(&sm);
    println!(
        "🔎 State-machine liveness check on `{}` ({} states, {} transitions)",
        state_enum,
        sm.states.len(),
        sm.transitions.len()
    );

    if report.is_clean() {
        println!("  ✅ All states reachable from creation and able to reach a terminal state.");
        return Ok(());
    }

    if !report.unreachable_from_initial.is_empty() {
        println!(
            "  ❌ Unreachable from creation (dead states): {}",
            report.unreachable_from_initial.join(", ")
        );
    }
    if !report.cannot_reach_terminal.is_empty() {
        println!(
            "  ❌ Cannot reach any terminal state (trapped): {}",
            report.cannot_reach_terminal.join(", ")
        );
    }
    if !report.stuck_states.is_empty() {
        println!(
            "  ❌ Stuck states / no terminal in machine: {}",
            report.stuck_states.join(", ")
        );
    }
    anyhow::bail!("state-machine liveness check failed");
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
    program: &intent_lang_syntax::ast::Program,
    source: &str,
) -> Result<()> {
    std::fs::create_dir_all(&cli.output_dir)?;

    // `.mmd` exports remain for Markdown embedding / download links; the
    // interactive `index.html` below is generated straight from `program` +
    // `source` (not by re-reading these files back), and is the same page
    // `--interactive` produces — see `html_generator` module docs.
    for (vis_kind, output) in render_all(program, cli.format.into())? {
        let filename = format!("{:?}.{}", vis_kind, extension_for_format(&cli.format));
        let path = cli.output_dir.join(filename.to_lowercase());
        std::fs::write(&path, output)?;
        eprintln!("✓ Generated {:?}", path);
    }

    let html = html_generator::generate_interactive_html(program, source)?;
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
