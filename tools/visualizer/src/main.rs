use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use intent_lang_visualizer::{
    analyze_state_machine, build_state_machine, build_state_machine_for, html_generator,
    lifecycle_enums, render, render_all, OutputFormat as LibOutputFormat, VisKind,
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
    Flowchart,
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
            VisType::Flowchart => VisKind::Flowchart,
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

/// Structural report for every declared lifecycle.
///
/// `intent check` is the gate — it applies the severity policy (which findings
/// are defects and which are legitimate modeling choices) and is available
/// wherever the verifier is. This flag stays as a quick read-out while looking
/// at diagrams, and reads the same derivation, so the two cannot disagree.
fn check_states(program: &intent_lang_syntax::ast::Program) -> Result<()> {
    println!("ℹ️  This is a read-out. `intent check --strict` is the gate.\n");

    let declared = lifecycle_enums(program);
    if declared.is_empty() {
        let sm = build_state_machine(program);
        match &sm.state_enum {
            Some(name) => println!(
                "⚠️  No `@lifecycle` declared. Showing `{name}` from the legacy heuristic — \
                 annotate it with `@lifecycle` to have `intent check` gate on it."
            ),
            None => println!(
                "ℹ️  No `@lifecycle` enum declared and none inferable — this file models no \
                 lifecycle."
            ),
        }
        return Ok(());
    }

    let mut failed = false;
    for enum_name in declared {
        let sm = build_state_machine_for(program, &enum_name);
        let report = analyze_state_machine(&sm);
        println!(
            "🔎 `{}` — {} states, {} transitions",
            enum_name,
            sm.states.len(),
            sm.transitions.len()
        );

        for c in &sm.conflicts {
            failed = true;
            println!(
                "  ❌ `{}` unconditionally asserts several next-states at once: {}",
                c.intent,
                c.targets.join(", ")
            );
        }
        if !report.has_creation {
            println!("  ⚠️  no creation edge — nothing can enter this lifecycle here");
        }
        if !report.unreachable_from_initial.is_empty() {
            failed = true;
            println!(
                "  ❌ unreachable from creation: {}",
                report.unreachable_from_initial.join(", ")
            );
        }
        if report.creation_targets.len() > 1 {
            println!(
                "  ⚠️  {} entry points: {} — a severed chain usually means a \
                 transition tests a boolean flag instead of the state field",
                report.creation_targets.len(),
                report.creation_targets.join(", ")
            );
        }
        if !report.has_terminal {
            println!("  ⚠️  no terminal state (fine for long-lived entities)");
        }
        if !report.cannot_reach_terminal.is_empty() {
            println!(
                "  ⚠️  cannot reach a terminal state: {}",
                report.cannot_reach_terminal.join(", ")
            );
        }
        if report.is_clean() && sm.conflicts.is_empty() {
            println!("  ✅ all states reachable from creation and able to terminate");
        }
    }

    if failed {
        anyhow::bail!("state-machine structural check failed");
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

    // One state-machine export per declared lifecycle. `render_all` covers the
    // primary one; a second lifecycle would otherwise have no file at all.
    let lifecycles = lifecycle_enums(program);
    if lifecycles.len() > 1 {
        for state_enum in &lifecycles {
            let output =
                intent_lang_visualizer::render_state_machine_of(program, state_enum, cli.format.into())?;
            let filename = format!(
                "statemachine-{}.{}",
                state_enum.to_lowercase(),
                extension_for_format(&cli.format)
            );
            let path = cli.output_dir.join(filename);
            std::fs::write(&path, output)?;
            eprintln!("✓ Generated {:?}", path);
        }
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
