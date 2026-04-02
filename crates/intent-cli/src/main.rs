use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use colored::Colorize;

use intent_core::DiagLevel;
use intent_core::smt::{verify_vc, VerifyResult};
use intent_core::typeck::check_program;
use intent_core::vcgen::{generate_vcs, VcKind};
use intent_syntax::parse;

#[derive(Parser)]
#[command(name = "intent", version, about = "intent-lang: formally verify your intents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse, type-check, and verify an .intent file
    Check {
        /// Path to .intent file
        file: PathBuf,
        /// Show SMT-LIB2 encoding (debug)
        #[arg(long)]
        show_smt: bool,
        /// Show applied safety rules
        #[arg(long)]
        show_safety: bool,
    },
    /// Parse and dump AST (debug)
    Parse {
        /// Path to .intent file
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check {
            file,
            show_smt,
            show_safety,
        } => cmd_check(&file, show_smt, show_safety),
        Commands::Parse { file } => cmd_parse(&file),
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

fn cmd_check(path: &PathBuf, show_smt: bool, show_safety: bool) {
    let source = read_file(path);
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    println!(
        "\n  {} {}...\n",
        "Checking".bold(),
        filename.cyan()
    );

    // Parse
    let prog = match parse(&source) {
        Ok(p) => p,
        Err(e) => {
            let (line, col) = offset_to_line_col(&source, e.span.start);
            eprintln!(
                "  {} {}\n    --> {}:{}:{}\n",
                "❌".red(),
                e.message,
                filename,
                line,
                col
            );
            process::exit(1);
        }
    };

    // Type check
    let diags = check_program(&prog);
    let has_errors = diags.iter().any(|d| d.level == DiagLevel::Error);
    for d in &diags {
        let (line, col) = offset_to_line_col(&source, d.span.start);
        let prefix = match d.level {
            DiagLevel::Error => "❌".red().to_string(),
            DiagLevel::Warning => "⚠️".yellow().to_string(),
            DiagLevel::Info => "ℹ️".blue().to_string(),
        };
        eprintln!(
            "  {} {}[{}]: {}\n    --> {}:{}:{}\n",
            prefix, 
            match d.level {
                DiagLevel::Error => "error".red().bold().to_string(),
                DiagLevel::Warning => "warning".yellow().bold().to_string(),
                DiagLevel::Info => "info".blue().bold().to_string(),
            },
            d.code,
            d.message,
            filename,
            line,
            col
        );
    }
    if has_errors {
        process::exit(1);
    }

    // Generate VCs and verify
    let vcs = generate_vcs(&prog);

    if vcs.is_empty() {
        println!("  {} no intents or theorems to verify\n", "ℹ️".blue());
        return;
    }

    let mut all_ok = true;

    for vc in &vcs {
        let kind_str = match vc.kind {
            VcKind::Intent => "intent",
            VcKind::Theorem => "theorem",
        };

        if show_safety && !vc.safety_rules.is_empty() {
            println!(
                "  {} applied safety rules for {}:",
                "ℹ️".blue(),
                vc.name.cyan()
            );
            for rule in &vc.safety_rules {
                println!(
                    "    - {}.invariant[{}]",
                    rule.safety_name, rule.index
                );
            }
            println!();
        }

        if show_smt {
            let mut encoder = intent_core::smt::SmtEncoder::new(&prog);
            encoder.encode_vc(vc, &prog);
            println!(
                "  {} SMT for {}:\n{}\n",
                "🔍".blue(),
                vc.name.cyan(),
                encoder.get_output()
            );
        }

        // Skip unsupported VCs with a warning
        if let Some(reason) = &vc.unsupported {
            println!(
                "  {} {} {} — {} ({})",
                "⚠️".yellow(),
                kind_str,
                vc.name.yellow().bold(),
                "skipped".yellow(),
                reason
            );
            continue;
        }

        let result = verify_vc(vc, &prog);

        match result {
            VerifyResult::Verified => {
                println!(
                    "  {} {} {} — {}",
                    "✅".green(),
                    kind_str,
                    vc.name.green().bold(),
                    "verified".green()
                );
            }
            VerifyResult::Failed { counterexample } => {
                all_ok = false;
                println!(
                    "  {} {} {} — {}",
                    "❌".red(),
                    kind_str,
                    vc.name.red().bold(),
                    "FAILED".red().bold()
                );
                if !counterexample.is_empty() {
                    println!(
                        "\n     {}",
                        "Counterexample:".yellow()
                    );
                    for line in counterexample.lines().take(20) {
                        println!("       {line}");
                    }
                    println!();
                }
            }
            VerifyResult::Unknown { reason } => {
                all_ok = false;
                println!(
                    "  {} {} {} — {} ({})",
                    "⚠️".yellow(),
                    kind_str,
                    vc.name.yellow().bold(),
                    "unknown".yellow(),
                    reason.lines().next().unwrap_or("")
                );
            }
            VerifyResult::Error { message } => {
                all_ok = false;
                println!(
                    "  {} {} {} — {}",
                    "❌".red(),
                    kind_str,
                    vc.name.red().bold(),
                    message.red()
                );
            }
        }
    }

    println!();

    if !all_ok {
        process::exit(1);
    }
}

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
