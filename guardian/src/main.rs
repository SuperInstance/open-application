use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod analyzer;
mod budget;
mod delta;
mod detector;
mod report;

#[derive(Parser)]
#[command(name = "guardian")]
#[command(about = "App Size Guardian — track binary size, enforce budgets, detect bloat")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a binary and print a conservation report
    Analyze {
        /// Path to the app binary or bundle directory
        #[arg(value_name = "PATH")]
        target: PathBuf,
        /// Path to the project's Cargo.toml (for dependency analysis)
        #[arg(long)]
        cargo_toml: Option<PathBuf>,
        /// Path to the assets directory (for asset size breakdown)
        #[arg(long)]
        assets_dir: Option<PathBuf>,
        /// Output report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Check binary against a size budget
    Check {
        /// Path to the app binary or bundle directory
        #[arg(value_name = "PATH")]
        target: PathBuf,
        /// Path to budget config (TOML)
        #[arg(long)]
        budget: PathBuf,
        /// Path to the project's Cargo.toml
        #[arg(long)]
        cargo_toml: Option<PathBuf>,
        /// Path to the assets directory
        #[arg(long)]
        assets_dir: Option<PathBuf>,
    },
    /// Compare two builds and show what changed
    Delta {
        /// Previous build artifact (binary or directory)
        #[arg(long)]
        before: PathBuf,
        /// Current build artifact (binary or directory)
        #[arg(long)]
        after: PathBuf,
    },
    /// Initialize a default budget file
    Init {
        /// Where to write the budget file
        #[arg(default_value = "guardian-budget.toml")]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            target,
            cargo_toml,
            assets_dir,
            json,
        } => {
            let analysis = analyzer::analyze(&target, cargo_toml.as_deref(), assets_dir.as_deref())
                .context("Failed to analyze binary")?;
            if json {
                println!("{}", serde_json::to_string_pretty(&analysis)?);
            } else {
                let rpt = report::Report::from_analysis(&analysis);
                rpt.print();
            }
        }
        Commands::Check {
            target,
            budget,
            cargo_toml,
            assets_dir,
        } => {
            let budget_config = budget::BudgetConfig::load(&budget)
                .context("Failed to load budget config")?;
            let budget = budget::SizeBudget::from_config(&budget_config);
            let analysis = analyzer::analyze(&target, cargo_toml.as_deref(), assets_dir.as_deref())
                .context("Failed to analyze binary")?;
            let violations = budget.check(&analysis);
            if violations.is_empty() {
                println!("{}", colored::Colorize::green("✓ All size budgets pass"));
            } else {
                for v in &violations {
                    eprintln!("{}", v);
                }
                std::process::exit(1);
            }
        }
        Commands::Delta { before, after } => {
            let before_analysis =
                analyzer::analyze(&before, None, None).context("Failed to analyze before")?;
            let after_analysis =
                analyzer::analyze(&after, None, None).context("Failed to analyze after")?;
            let d = delta::Delta::compute(&before_analysis, &after_analysis);
            d.print_report();
        }
        Commands::Init { output } => {
            let default_config = budget::BudgetConfig::default();
            let toml_str = toml::to_string_pretty(&default_config)?;
            std::fs::write(&output, &toml_str)
                .context(format!("Failed to write budget to {}", output.display()))?;
            println!("Created {}", output.display());
        }
    }

    Ok(())
}
