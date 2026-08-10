mod facts;

use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use serde::Serialize;

use intent_lang_core::analysis::{
    coverage_report, diff as ana_diff, explain as ana_explain, impact as ana_impact,
    testspec as ana_testspec, Change, Lifecycle, ModificationKind,
};
use intent_lang_core::smt::{verify_vc, VerifyResult};
use intent_lang_core::typeck::check_program;
use intent_lang_core::vcgen::{generate_vcs, VcKind};
use intent_lang_core::{DiagLevel, Diagnostic};
use intent_lang_syntax::ast::Declaration;
use intent_lang_syntax::parse;

#[derive(Parser)]
#[command(
    name = "intent",
    version,
    about = "intent-lang: requirements modeling DSL with formal verification"
)]
struct Cli {
    /// Output format (applies to all subcommands)
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse, type-check, and verify an .intent file
    Check {
        file: PathBuf,
        /// Show SMT-LIB2 encoding (debug)
        #[arg(long)]
        show_smt: bool,
        /// Show applied safety rules
        #[arg(long)]
        show_safety: bool,
        /// Include @asis intents (default: skip them)
        #[arg(long)]
        include_asis: bool,
        /// Treat ambiguous structural findings (unclaimed goals, missing
        /// creation edge / terminal state, missing examples) as errors.
        /// Off by default because the tool cannot tell those apart from
        /// legitimate modeling choices; skills turn it on.
        #[arg(long)]
        strict: bool,
    },
    /// Parse and dump AST (debug)
    Parse { file: PathBuf },
    /// Run completeness analysis on `coverage` blocks
    Coverage { file: PathBuf },
    /// Emit a test specification (scenarios per intent) — for downstream tools
    Testspec { file: PathBuf },
    /// Diff two .intent files; classify changes (loosened / tightened / reshaped)
    Diff { old: PathBuf, new: PathBuf },
    /// Walk diff to identify affected goals and coverage scenarios
    Impact { old: PathBuf, new: PathBuf },
    /// Render a plain-English explanation of an intent / safety / goal
    Explain { file: PathBuf, target: String },
    /// Audit a .intent against the facts.md it was translated from:
    /// which confirmed facts never became a clause, and which referenced
    /// fact_ids are dangling or unconfirmed
    Trace {
        file: PathBuf,
        /// Facts document (defaults to <domain>.facts.md next to the .intent)
        #[arg(long)]
        facts: Option<PathBuf>,
    },
    /// Executable acceptance pipeline (RFC: executable-acceptance)
    Accept {
        #[command(subcommand)]
        command: AcceptCommands,
    },
}

#[derive(Subcommand)]
enum AcceptCommands {
    /// Generate pytest file + manifest from .intent + binding (deterministic)
    Gen {
        file: PathBuf,
        /// Binding file (defaults to <file>.bind.toml)
        #[arg(long)]
        binding: Option<PathBuf>,
        /// Output directory for generated tests
        #[arg(long, default_value = "intent-accept")]
        out: PathBuf,
    },
    /// gen + run pytest + merge results into intent.acceptance_report
    Run {
        file: PathBuf,
        /// Binding file (defaults to <file>.bind.toml)
        #[arg(long)]
        binding: Option<PathBuf>,
        /// Output directory for generated tests and report
        #[arg(long, default_value = "intent-accept")]
        out: PathBuf,
        /// Gate mode: strict (manual-pending fails too) or lenient
        #[arg(long, value_enum, default_value_t = GateModeArg::Strict)]
        gate: GateModeArg,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum GateModeArg {
    Strict,
    Lenient,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            file,
            show_smt,
            show_safety,
            include_asis,
            strict,
        } => cmd_check(
            &file,
            show_smt,
            show_safety,
            include_asis,
            strict,
            cli.format,
        ),
        Commands::Parse { file } => cmd_parse(&file),
        Commands::Coverage { file } => cmd_coverage(&file, cli.format),
        Commands::Testspec { file } => cmd_testspec(&file, cli.format),
        Commands::Diff { old, new } => cmd_diff(&old, &new, cli.format),
        Commands::Impact { old, new } => cmd_impact(&old, &new, cli.format),
        Commands::Explain { file, target } => cmd_explain(&file, &target, cli.format),
        Commands::Trace { file, facts } => cmd_trace(&file, facts, cli.format),
        Commands::Accept { command } => match command {
            AcceptCommands::Gen { file, binding, out } => cmd_accept_gen(&file, binding, &out),
            AcceptCommands::Run {
                file,
                binding,
                out,
                gate,
            } => cmd_accept_run(&file, binding, &out, gate, cli.format),
        },
    }
}

fn read_file(path: &PathBuf) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{} cannot read {}: {e}",
                "error:".red().bold(),
                path.display()
            );
            process::exit(1);
        }
    }
}

fn parse_or_die(path: &PathBuf) -> intent_lang_syntax::ast::Program {
    let source = read_file(path);
    match parse(&source) {
        Ok(p) => p,
        Err(e) => {
            let (line, col) = offset_to_line_col(&source, e.span.start);
            eprintln!(
                "  {} {} (at {}:{}:{})",
                "❌".red(),
                e.message,
                path.display(),
                line,
                col
            );
            process::exit(1);
        }
    }
}

// ── check ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct CheckJson {
    file: String,
    diagnostics: Vec<DiagJson>,
    results: Vec<VcJson>,
    /// Structural-check roll-up, for tracking whether the modeling gate is
    /// catching anything (RFC: workflow-hardening §5.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    structure: Option<intent_lang_core::structure::StructureSummary>,
    ok: bool,
}

#[derive(Serialize)]
struct DiagJson {
    level: String,
    code: String,
    message: String,
    line: usize,
    col: usize,
}

#[derive(Serialize)]
struct VcJson {
    name: String,
    kind: String,
    status: String,
    detail: Option<String>,
    /// One of: "primary", "asis-skipped"
    track: String,
}

/// Render one diagnostic to stderr (text mode) and to the JSON accumulator.
fn emit_diag(
    d: &Diagnostic,
    source: &str,
    filename: &str,
    fmt: OutputFormat,
    out: &mut Vec<DiagJson>,
) {
    let (line, col) = offset_to_line_col(source, d.span.start);
    let level_str = match d.level {
        DiagLevel::Error => "error",
        DiagLevel::Warning => "warning",
        DiagLevel::Info => "info",
    };
    out.push(DiagJson {
        level: level_str.to_string(),
        code: d.code.clone(),
        message: d.message.clone(),
        line,
        col,
    });
    if matches!(fmt, OutputFormat::Text) {
        let (icon, label) = match d.level {
            DiagLevel::Error => ("❌".red().to_string(), "error".red().bold().to_string()),
            DiagLevel::Warning => (
                "⚠️".yellow().to_string(),
                "warning".yellow().bold().to_string(),
            ),
            DiagLevel::Info => ("ℹ️".blue().to_string(), "info".blue().bold().to_string()),
        };
        eprintln!(
            "  {} {}[{}]: {}\n    --> {}:{}:{}",
            icon, label, d.code, d.message, filename, line, col
        );
        for note in &d.notes {
            eprintln!("    {} {note}", "=".dimmed());
        }
        eprintln!();
    }
}

fn cmd_check(
    path: &PathBuf,
    show_smt: bool,
    show_safety: bool,
    include_asis: bool,
    strict: bool,
    fmt: OutputFormat,
) {
    let source = read_file(path);
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    if matches!(fmt, OutputFormat::Text) {
        println!("\n  {} {}...\n", "Checking".bold(), filename.cyan());
    }

    let prog = match parse(&source) {
        Ok(p) => p,
        Err(e) => {
            let (line, col) = offset_to_line_col(&source, e.span.start);
            match fmt {
                OutputFormat::Text => {
                    eprintln!(
                        "  {} {}\n    --> {}:{}:{}\n",
                        "❌".red(),
                        e.message,
                        filename,
                        line,
                        col
                    );
                }
                OutputFormat::Json => {
                    let out = CheckJson {
                        file: filename.to_string(),
                        diagnostics: vec![DiagJson {
                            level: "error".to_string(),
                            code: "PARSE".to_string(),
                            message: e.message,
                            line,
                            col,
                        }],
                        results: vec![],
                        structure: None,
                        ok: false,
                    };
                    println!("{}", serde_json::to_string_pretty(&out).unwrap());
                }
            }
            process::exit(1);
        }
    };

    let diags = check_program(&prog);
    let has_errors = diags.iter().any(|d| d.level == DiagLevel::Error);
    let mut diag_jsons = Vec::new();
    for d in &diags {
        emit_diag(d, &source, &filename, fmt, &mut diag_jsons);
    }
    if has_errors {
        if matches!(fmt, OutputFormat::Json) {
            let out = CheckJson {
                file: filename.to_string(),
                diagnostics: diag_jsons,
                results: vec![],
                structure: None,
                ok: false,
            };
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        process::exit(1);
    }

    // Structural checks (RFC: workflow-hardening D1). Run after type-checking
    // — they read a well-formed AST — and before verification, so a file that
    // models nothing coherent says so before printing a wall of green VCs.
    let (structure_diags, structure_summary) =
        intent_lang_core::structure::check_structure(&prog, strict);
    let structure_failed = structure_diags
        .iter()
        .any(|d| d.level == DiagLevel::Error);
    for d in &structure_diags {
        emit_diag(d, &source, &filename, fmt, &mut diag_jsons);
    }

    // Build asis exclusion set (RFC A2)
    let asis_intents: std::collections::HashSet<String> = prog
        .declarations
        .iter()
        .filter_map(|d| match &d.node {
            Declaration::Intent(i)
                if matches!(intent_lang_core::analysis::intent_lifecycle(i), Lifecycle::AsIs) =>
            {
                Some(i.name.clone())
            }
            _ => None,
        })
        .collect();

    let vcs = generate_vcs(&prog);

    let mut all_ok = true;
    let mut vc_jsons = Vec::new();

    for vc in &vcs {
        let kind_str = match vc.kind {
            VcKind::Intent => "intent",
            VcKind::Theorem => "theorem",
        };

        let is_asis = asis_intents.contains(&vc.name);
        let track = if is_asis { "asis-skipped" } else { "primary" };

        if is_asis && !include_asis {
            if matches!(fmt, OutputFormat::Text) {
                println!(
                    "  {} {} {} — {} (legacy track; pass --include-asis to verify)",
                    "🟡".yellow(),
                    kind_str,
                    vc.name.yellow().bold(),
                    "skipped".yellow()
                );
            }
            vc_jsons.push(VcJson {
                name: vc.name.clone(),
                kind: kind_str.to_string(),
                status: "asis-skipped".to_string(),
                detail: None,
                track: track.to_string(),
            });
            continue;
        }

        if matches!(fmt, OutputFormat::Text) && show_safety && !vc.safety_rules.is_empty() {
            println!(
                "  {} applied safety rules for {}:",
                "ℹ️".blue(),
                vc.name.cyan()
            );
            for rule in &vc.safety_rules {
                println!("    - {}.invariant[{}]", rule.safety_name, rule.index);
            }
            println!();
        }

        if matches!(fmt, OutputFormat::Text) && show_smt {
            let mut encoder = intent_lang_core::smt::SmtEncoder::new(&prog);
            encoder.encode_vc(vc, &prog);
            println!(
                "  {} SMT for {}:\n{}\n",
                "🔍".blue(),
                vc.name.cyan(),
                encoder.get_output()
            );
        }

        if let Some(reason) = &vc.unsupported {
            if matches!(fmt, OutputFormat::Text) {
                println!(
                    "  {} {} {} — {} ({})",
                    "⚠️".yellow(),
                    kind_str,
                    vc.name.yellow().bold(),
                    "skipped".yellow(),
                    reason
                );
            }
            vc_jsons.push(VcJson {
                name: vc.name.clone(),
                kind: kind_str.to_string(),
                status: "skipped".to_string(),
                detail: Some(reason.clone()),
                track: track.to_string(),
            });
            continue;
        }

        let result = verify_vc(vc, &prog);
        let (status, detail) = match &result {
            VerifyResult::Verified => ("verified".to_string(), None),
            VerifyResult::Failed { counterexample } => {
                all_ok = false;
                ("failed".to_string(), Some(counterexample.clone()))
            }
            VerifyResult::Unknown { reason } => {
                all_ok = false;
                ("unknown".to_string(), Some(reason.clone()))
            }
            VerifyResult::Error { message } => {
                all_ok = false;
                ("error".to_string(), Some(message.clone()))
            }
            VerifyResult::SelfContradictory => {
                all_ok = false;
                (
                    "self-contradictory".to_string(),
                    Some(
                        "V0020: clauses are unsatisfiable — no state can ever \
                         satisfy this intent (vacuous verification rejected)"
                            .to_string(),
                    ),
                )
            }
        };

        if matches!(fmt, OutputFormat::Text) {
            match &result {
                VerifyResult::Verified => println!(
                    "  {} {} {} — {}",
                    "✅".green(),
                    kind_str,
                    vc.name.green().bold(),
                    "verified".green()
                ),
                VerifyResult::Failed { counterexample } => {
                    println!(
                        "  {} {} {} — {}",
                        "❌".red(),
                        kind_str,
                        vc.name.red().bold(),
                        "FAILED".red().bold()
                    );
                    if !counterexample.is_empty() {
                        println!("\n     {}", "Counterexample:".yellow());
                        for line in counterexample.lines().take(20) {
                            println!("       {line}");
                        }
                        println!();
                    }
                }
                VerifyResult::Unknown { reason } => println!(
                    "  {} {} {} — {} ({})",
                    "⚠️".yellow(),
                    kind_str,
                    vc.name.yellow().bold(),
                    "unknown".yellow(),
                    reason.lines().next().unwrap_or("")
                ),
                VerifyResult::Error { message } => println!(
                    "  {} {} {} — {}",
                    "❌".red(),
                    kind_str,
                    vc.name.red().bold(),
                    message.red()
                ),
                VerifyResult::SelfContradictory => {
                    println!(
                        "  {} {} {} — {}",
                        "❌".red(),
                        kind_str,
                        vc.name.red().bold(),
                        "SELF-CONTRADICTORY".red().bold()
                    );
                    println!(
                        "\n     {} error[V0020]: the intent's own clauses can never hold\n     simultaneously; any \"verified\" result would be vacuous.\n",
                        "⚠".yellow()
                    );
                }
            }
        }

        vc_jsons.push(VcJson {
            name: vc.name.clone(),
            kind: kind_str.to_string(),
            status,
            detail,
            track: track.to_string(),
        });
    }

    // D5: check `example` blocks against their intents via Z3.
    let example_results = intent_lang_core::example::check_examples(&prog);
    for er in &example_results {
        use intent_lang_core::example::ExampleStatus;
        let title = er.title.clone().unwrap_or_default();
        let (status, detail) = match &er.status {
            ExampleStatus::Consistent => {
                if matches!(fmt, OutputFormat::Text) {
                    println!(
                        "  {} example {} {} — {}",
                        "✅".green(),
                        er.intent.green().bold(),
                        title.dimmed(),
                        "consistent".green()
                    );
                }
                ("verified".to_string(), None)
            }
            ExampleStatus::Violates { clause_id, clause } => {
                all_ok = false;
                if matches!(fmt, OutputFormat::Text) {
                    println!(
                        "  {} example {} {} — {}",
                        "❌".red(),
                        er.intent.red().bold(),
                        title.dimmed(),
                        "VIOLATES CLAUSE".red().bold()
                    );
                    println!(
                        "\n     error[V0021]: example contradicts `{}`:\n       {}\n     the formula and the author's concrete expectation disagree — one of them is wrong.\n",
                        clause_id.yellow(),
                        clause
                    );
                }
                (
                    "failed".to_string(),
                    Some(format!("V0021: example contradicts {clause_id}: {clause}")),
                )
            }
            ExampleStatus::Inconsistent => {
                all_ok = false;
                if matches!(fmt, OutputFormat::Text) {
                    println!(
                        "  {} example {} {} — {}",
                        "❌".red(),
                        er.intent.red().bold(),
                        title.dimmed(),
                        "INCONSISTENT with clause set".red().bold()
                    );
                }
                (
                    "failed".to_string(),
                    Some("V0021: example inconsistent with full clause set".to_string()),
                )
            }
            ExampleStatus::Unknown { reason } => {
                all_ok = false;
                if matches!(fmt, OutputFormat::Text) {
                    println!(
                        "  {} example {} — {} ({})",
                        "⚠️".yellow(),
                        er.intent.yellow().bold(),
                        "unknown".yellow(),
                        reason
                    );
                }
                ("unknown".to_string(), Some(reason.clone()))
            }
        };
        vc_jsons.push(VcJson {
            name: format!("example {}", er.intent),
            kind: "example".to_string(),
            status,
            detail,
            track: "primary".to_string(),
        });
    }

    if matches!(fmt, OutputFormat::Text) {
        println!();
    }

    let ok = all_ok && !structure_failed;

    if matches!(fmt, OutputFormat::Json) {
        let out = CheckJson {
            file: filename.to_string(),
            diagnostics: diag_jsons,
            results: vc_jsons,
            structure: Some(structure_summary),
            ok,
        };
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    }

    if !ok {
        process::exit(1);
    }
}

// ── trace ───────────────────────────────────────────────────────

fn cmd_trace(path: &PathBuf, facts_arg: Option<PathBuf>, fmt: OutputFormat) {
    let intent_source = read_file(path);
    let facts_path = facts_arg.unwrap_or_else(|| facts::conventional_facts_path(path));

    let facts_source = match std::fs::read_to_string(&facts_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{} cannot read facts document {}: {e}\n  {} by convention a facts document sits \
                 next to its .intent and is named after the same domain, e.g. \
                 `<domain>.facts.md`. Pass --facts <path> to override.",
                "error:".red().bold(),
                facts_path.display(),
                "=".dimmed(),
            );
            process::exit(1);
        }
    };

    let report = facts::audit(
        &path.file_name().unwrap_or_default().to_string_lossy(),
        &facts_path.file_name().unwrap_or_default().to_string_lossy(),
        &intent_source,
        &facts_source,
    );

    match fmt {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        OutputFormat::Text => print_trace_report(&report),
    }

    if !report.ok() {
        process::exit(1);
    }
}

fn print_trace_report(report: &facts::TraceReport) {
    println!(
        "\n  {} {} against {}\n",
        "Tracing".bold(),
        report.intent_file.cyan(),
        report.facts_file.cyan()
    );

    // Stated up front and unconditionally: a parser that silently understood
    // fewer facts than the document holds would otherwise be indistinguishable
    // from a translation that dropped them.
    let by_kind: Vec<String> = report
        .parsed
        .iter()
        .map(|(k, n)| format!("{n} {k}"))
        .collect();
    println!(
        "  {} parsed {} facts ({})",
        "ℹ️".blue(),
        report.parsed_total.to_string().bold(),
        if by_kind.is_empty() {
            "none".to_string()
        } else {
            by_kind.join(" / ")
        }
    );
    let by_status: Vec<String> = report
        .statuses
        .iter()
        .map(|(k, n)| format!("{n} {k}"))
        .collect();
    println!("     review status: {}\n", by_status.join(" / "));

    for w in &report.parse_warnings {
        println!(
            "  {} {} (line {})",
            "⚠️".yellow(),
            w.text.yellow(),
            w.line
        );
    }
    if !report.parse_warnings.is_empty() {
        println!();
    }

    let section = |title: &str, facts: &[facts::Fact]| {
        if facts.is_empty() {
            return;
        }
        println!("  {} {}", "❌".red(), title.red().bold());
        for f in facts {
            println!(
                "       {}  {}",
                f.id.yellow(),
                truncate(&f.statement, 68).dimmed()
            );
        }
        println!();
    };

    section(
        "confirmed facts with no clause in the .intent:",
        &report.confirmed_without_clause,
    );
    section(
        "SUS/UNK facts still in draft — the confirmation gate was skipped:",
        &report.undecided_suspicions,
    );
    section(
        "referenced facts that are not confirmed:",
        &report.references_not_confirmed,
    );

    if !report.dangling_references.is_empty() {
        println!(
            "  {} {}",
            "❌".red(),
            "fact_ids referenced by the .intent but absent from the facts document:"
                .red()
                .bold()
        );
        for id in &report.dangling_references {
            println!("       {}", id.yellow());
        }
        println!();
    }

    if report.ok() {
        println!(
            "  {} every confirmed fact maps to a clause; no undecided suspicions\n",
            "✅".green()
        );
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let head: String = s.chars().take(max_chars).collect();
    format!("{head}…")
}

// ── coverage ────────────────────────────────────────────────────

fn cmd_coverage(path: &PathBuf, fmt: OutputFormat) {
    let prog = parse_or_die(path);
    let _ = check_program(&prog);
    let report = coverage_report(&prog);
    match fmt {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
        OutputFormat::Text => {
            println!("\n  {} {}\n", "Coverage".bold(), path.display());
            if report.coverages.is_empty() {
                println!(
                    "  {} no `coverage` declarations found in this file",
                    "ℹ️".blue()
                );
                return;
            }
            for s in &report.coverages {
                println!(
                    "  {} {} — {}/{} combinations covered",
                    if s.uncovered.is_empty() {
                        "✅".green().to_string()
                    } else {
                        "⚠️".yellow().to_string()
                    },
                    s.name.cyan().bold(),
                    s.covered,
                    s.total_combinations
                );
                if !s.uncovered.is_empty() {
                    println!("    {}", "Uncovered combinations:".yellow());
                    for combo in s.uncovered.iter().take(20) {
                        let parts: Vec<String> =
                            combo.iter().map(|(k, v)| format!("{k}={v}")).collect();
                        println!("      • {}", parts.join(", "));
                    }
                    if s.uncovered.len() > 20 {
                        println!("      … {} more", s.uncovered.len() - 20);
                    }
                }
                println!();
            }
        }
    }
}

// ── testspec ────────────────────────────────────────────────────

fn cmd_testspec(path: &PathBuf, fmt: OutputFormat) {
    let prog = parse_or_die(path);
    let _ = check_program(&prog);
    let spec = ana_testspec(&prog);
    match fmt {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&spec).unwrap());
        }
        OutputFormat::Text => {
            println!("\n  {} {}\n", "Testspec".bold(), path.display());
            for it in &spec.intents {
                let lc = match it.lifecycle {
                    Lifecycle::AsIs => "[asis]".yellow().to_string(),
                    Lifecycle::ToBe => "[tobe]".cyan().to_string(),
                    Lifecycle::Current => "".to_string(),
                };
                println!("  {} {} {}", "intent".bold(), it.intent.cyan(), lc);
                println!("    params: {}", it.params.join(", "));
                for (i, sc) in it.scenarios.iter().enumerate() {
                    println!("    {:2}. {}", i + 1, sc.label.bold());
                    if !sc.assumptions.is_empty() {
                        println!("        given: {}", sc.assumptions.join(" && "));
                    }
                    println!("        expect: {}", sc.expected);
                }
                println!();
            }
        }
    }
}

// ── diff ────────────────────────────────────────────────────────

fn cmd_diff(old: &PathBuf, new: &PathBuf, fmt: OutputFormat) {
    let p_old = parse_or_die(old);
    let p_new = parse_or_die(new);
    let report = ana_diff(&p_old, &p_new);
    match fmt {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
        OutputFormat::Text => print_diff(&report),
    }
}

fn print_diff(r: &intent_lang_core::analysis::DiffReport) {
    println!(
        "\n  {} {} added · {} removed · {} modified · {} potentially-breaking\n",
        "Diff:".bold(),
        r.summary.added,
        r.summary.removed,
        r.summary.modified,
        r.summary.potentially_breaking.to_string().red().bold()
    );
    for c in &r.changes {
        match c {
            Change::Added { decl_kind, name } => {
                println!("  {} {} {}", "➕".green(), decl_kind, name.green().bold());
            }
            Change::Removed { decl_kind, name } => {
                println!("  {} {} {}", "➖".red(), decl_kind, name.red().bold());
            }
            Change::Modified {
                decl_kind,
                name,
                classification,
                details,
            } => {
                let label = match classification {
                    ModificationKind::Loosened => "loosened".green().to_string(),
                    ModificationKind::Tightened => "TIGHTENED".red().bold().to_string(),
                    ModificationKind::Reshaped => "reshaped".yellow().to_string(),
                };
                println!(
                    "  {} {} {} — {}",
                    "✎".yellow(),
                    decl_kind,
                    name.cyan().bold(),
                    label
                );
                for d in details {
                    println!("      {d}");
                }
            }
        }
    }
    println!();
}

// ── impact ──────────────────────────────────────────────────────

fn cmd_impact(old: &PathBuf, new: &PathBuf, fmt: OutputFormat) {
    let p_old = parse_or_die(old);
    let p_new = parse_or_die(new);
    let report = ana_impact(&p_old, &p_new);
    match fmt {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
        OutputFormat::Text => {
            print_diff(&report.diff);
            println!(
                "  {} {}",
                "Affected goals:".bold(),
                if report.affected_goals.is_empty() {
                    "(none)".to_string()
                } else {
                    report.affected_goals.join(", ")
                }
            );
            println!(
                "  {} {}",
                "Affected coverages:".bold(),
                if report.affected_coverages.is_empty() {
                    "(none)".to_string()
                } else {
                    report.affected_coverages.join(", ")
                }
            );
            println!();
        }
    }
}

// ── explain ─────────────────────────────────────────────────────

fn cmd_explain(path: &PathBuf, target: &str, fmt: OutputFormat) {
    let prog = parse_or_die(path);
    let _ = check_program(&prog);
    match ana_explain(&prog, target) {
        None => {
            eprintln!(
                "  {} no intent/safety/goal named `{}` in {}",
                "❌".red(),
                target,
                path.display()
            );
            process::exit(1);
        }
        Some(report) => match fmt {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            }
            OutputFormat::Text => {
                println!(
                    "\n  {} {} ({})\n",
                    "Explain".bold(),
                    report.target.cyan().bold(),
                    report.kind
                );
                println!("  {}\n", report.plain_english);
                if !report.clauses.is_empty() {
                    println!("  {}", "Clauses:".bold());
                    for c in &report.clauses {
                        println!("    • [{}] {}", c.kind, c.formal);
                        println!("        ↳ {}", c.natural);
                    }
                    println!();
                }
                if let Some(e) = &report.satisfying_example {
                    println!("  {} {}", "Satisfying example:".green(), e);
                }
                if let Some(e) = &report.violating_example {
                    println!("  {} {}", "Violating example:".red(), e);
                }
                println!();
            }
        },
    }
}

// ── accept (RFC: executable-acceptance, M-A1) ───────────────────

fn default_binding_path(file: &PathBuf) -> PathBuf {
    let mut p = file.clone();
    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
    p.set_file_name(format!("{name}.bind.toml"));
    p
}

/// Shared front half of gen/run: parse, typecheck (errors are fatal —
/// goal A is the gate for goal B), verify (V0020 must block acceptance),
/// load binding, generate tests + manifest, write them to `out`.
fn accept_generate(
    file: &PathBuf,
    binding_path: &PathBuf,
    out: &PathBuf,
) -> (
    intent_lang_accept::codegen::Manifest,
    PathBuf, // test file path
) {
    let prog = parse_or_die(file);
    let diags = check_program(&prog);
    let mut fatal = false;
    for d in diags
        .iter()
        .filter(|d| d.level == DiagLevel::Error)
    {
        eprintln!("  {} {}", "❌".red(), d);
        fatal = true;
    }
    if fatal {
        eprintln!(
            "\n  {} requirements must typecheck before acceptance (goal A gates goal B)",
            "error:".red().bold()
        );
        process::exit(2);
    }

    let binding = match intent_lang_accept::binding::load_binding(binding_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("  {} {e}", "❌".red());
            process::exit(2);
        }
    };

    let source_dir = file
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let gen = intent_lang_accept::codegen::generate(
        &prog,
        &binding,
        &file.to_string_lossy(),
        &binding_path.to_string_lossy(),
        &source_dir.to_string_lossy(),
    );

    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("  {} cannot create {}: {e}", "❌".red(), out.display());
        process::exit(2);
    }
    let test_path = out.join("test_acceptance.py");
    std::fs::write(&test_path, &gen.pytest_code).expect("write test file");
    let manifest_path = out.join("acceptance_manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&gen.manifest).unwrap(),
    )
    .expect("write manifest");

    (gen.manifest, test_path)
}

fn cmd_accept_gen(file: &PathBuf, binding: Option<PathBuf>, out: &PathBuf) {
    let binding_path = binding.unwrap_or_else(|| default_binding_path(file));
    let (manifest, test_path) = accept_generate(file, &binding_path, out);

    println!(
        "\n  {} {} test(s), {} machine clause(s), {} manual item(s)",
        "Generated".bold(),
        manifest.tests.len().to_string().cyan(),
        manifest.machine_clause_ids.len().to_string().cyan(),
        manifest.manual_items.len().to_string().yellow()
    );
    println!("    tests:    {}", test_path.display());
    println!(
        "    manifest: {}",
        out.join("acceptance_manifest.json").display()
    );
    if !manifest.manual_items.is_empty() {
        println!("\n  {}", "Manual checklist (D7 — never silently skipped):".yellow().bold());
        for m in &manifest.manual_items {
            println!("    • {} — {}", m.clause_id.yellow(), m.reason);
        }
    }
    println!();
}

fn cmd_accept_run(
    file: &PathBuf,
    binding: Option<PathBuf>,
    out: &PathBuf,
    gate: GateModeArg,
    fmt: OutputFormat,
) {
    use intent_lang_accept::report::{build_report, parse_junit, run_pytest, GateMode};

    let binding_path = binding.unwrap_or_else(|| default_binding_path(file));
    let (manifest, test_path) = accept_generate(file, &binding_path, out);

    let junit_xml = match run_pytest(&test_path, out) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("  {} {e}", "❌".red());
            process::exit(2);
        }
    };
    let results = parse_junit(&junit_xml);

    let mode = match gate {
        GateModeArg::Strict => GateMode::Strict,
        GateModeArg::Lenient => GateMode::Lenient,
    };
    let report = build_report(&manifest, &results, mode);

    let report_path = out.join("acceptance_report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&report).unwrap())
        .expect("write report");

    match fmt {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
        OutputFormat::Text => {
            println!(
                "\n  {} {} — adapter {}\n",
                "Acceptance".bold(),
                report.file.cyan(),
                report.adapter
            );
            for c in &report.clauses {
                match c.status.as_str() {
                    "passed" => println!(
                        "  {} {} — passed ({} scenario(s))",
                        "✅".green(),
                        c.id.green(),
                        c.scenarios
                    ),
                    "failed" => {
                        println!("  {} {} — {}", "❌".red(), c.id.red().bold(), "FAILED".red().bold());
                        if let Some(d) = &c.detail {
                            println!("       {}", d);
                        }
                        if let Some(s) = &c.scenario {
                            println!("       scenario: {s}");
                        }
                    }
                    "blocked" => println!(
                        "  {} {} — blocked ({})",
                        "⚠️".yellow(),
                        c.id.yellow(),
                        c.reason.as_deref().unwrap_or("")
                    ),
                    _ => println!(
                        "  {} {} — manual-pending ({})",
                        "🟡".yellow(),
                        c.id.yellow(),
                        c.reason.as_deref().unwrap_or("")
                    ),
                }
            }
            if !report.goals.is_empty() {
                println!("\n  {}", "Goals:".bold());
                for g in &report.goals {
                    println!(
                        "    • {} — machine {}/{} passed, {} manual pending",
                        g.name.cyan(),
                        g.machine.passed,
                        g.machine.total,
                        g.manual.pending
                    );
                }
            }
            println!(
                "\n  {} {} passed · {} failed · {} manual-pending · gate[{}] = {}\n",
                "Summary:".bold(),
                report.summary.passed.to_string().green(),
                report.summary.failed.to_string().red(),
                report.summary.manual_pending.to_string().yellow(),
                report.gate.mode,
                if report.gate.verdict == "pass" {
                    report.gate.verdict.green().bold().to_string()
                } else {
                    report.gate.verdict.red().bold().to_string()
                }
            );
            println!("    report: {}", report_path.display());
            println!();
        }
    }

    if report.gate.verdict == "fail" {
        process::exit(1);
    }
}

// ── parse ───────────────────────────────────────────────────────

fn cmd_parse(path: &PathBuf) {
    let source = read_file(path);
    match parse(&source) {
        Ok(prog) => {
            println!("{:#?}", prog);
        }
        Err(e) => {
            eprintln!("Parse error: {e}");
            process::exit(1);
        }
    }
}

fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
