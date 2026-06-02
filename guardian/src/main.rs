use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod alert;
mod analyzer;
mod budget;
mod delta;
mod detector;
mod export;
mod persistence;
mod report;
mod tauri_adapter;
mod trend;

#[derive(Parser)]
#[command(name = "guardian")]
#[command(version = "0.2.0")]
#[command(about = "App Size Guardian — track binary size, enforce budgets, detect bloat, analyze trends")]
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
        /// Path to tauri.conf.json for Tauri-specific analysis
        #[arg(long)]
        tauri_conf: Option<PathBuf>,
        /// Output report as JSON
        #[arg(long)]
        json: bool,
        /// Save analysis to history file for trend tracking
        #[arg(long)]
        save: bool,
        /// Path to history file (default: .guardian-history.json)
        #[arg(long, default_value = ".guardian-history.json")]
        history: PathBuf,
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
        /// Path to tauri.conf.json
        #[arg(long)]
        tauri_conf: Option<PathBuf>,
        /// Save analysis to history
        #[arg(long)]
        save: bool,
        #[arg(long, default_value = ".guardian-history.json")]
        history: PathBuf,
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
    /// Export analysis in various formats
    Export {
        /// Path to the app binary or bundle directory
        #[arg(value_name = "PATH")]
        target: PathBuf,
        /// Export format: json, prometheus, markdown, csv
        #[arg(long, value_name = "FORMAT")]
        format: String,
        /// Output file (stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Path to Cargo.toml
        #[arg(long)]
        cargo_toml: Option<PathBuf>,
        /// Path to assets directory
        #[arg(long)]
        assets_dir: Option<PathBuf>,
        /// Path to tauri.conf.json
        #[arg(long)]
        tauri_conf: Option<PathBuf>,
    },
    /// Show size trend across historical builds
    Trend {
        /// Path to history file
        #[arg(long, default_value = ".guardian-history.json")]
        history: PathBuf,
        /// Number of recent builds to show
        #[arg(long, default_value = "10")]
        limit: usize,
        /// Export trend as markdown table
        #[arg(long)]
        markdown: bool,
    },
    /// Show alerts based on history or current build
    Alerts {
        /// Path to the app binary or bundle directory
        #[arg(value_name = "PATH")]
        target: PathBuf,
        /// Path to history file
        #[arg(long, default_value = ".guardian-history.json")]
        history: PathBuf,
        /// Path to Cargo.toml
        #[arg(long)]
        cargo_toml: Option<PathBuf>,
        /// Path to assets directory
        #[arg(long)]
        assets_dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            target,
            cargo_toml,
            assets_dir,
            tauri_conf,
            json,
            save,
            history,
        } => {
            let mut analysis = analyzer::analyze(&target, cargo_toml.as_deref(), assets_dir.as_deref())
                .context("Failed to analyze binary")?;

            // Tauri-specific enrichment
            if let Some(tc) = &tauri_conf {
                tauri_adapter::enrich(tc, &mut analysis);
            }

            if save {
                persistence::append(&history, &analysis)?;
            }

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
            tauri_conf,
            save,
            history,
        } => {
            let budget_config = budget::BudgetConfig::load(&budget)
                .context("Failed to load budget config")?;
            let budget = budget::SizeBudget::from_config(&budget_config);
            let mut analysis = analyzer::analyze(&target, cargo_toml.as_deref(), assets_dir.as_deref())
                .context("Failed to analyze binary")?;

            if let Some(tc) = &tauri_conf {
                tauri_adapter::enrich(tc, &mut analysis);
            }

            if save {
                persistence::append(&history, &analysis)?;
            }

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
        Commands::Export {
            target,
            format,
            output,
            cargo_toml,
            assets_dir,
            tauri_conf,
        } => {
            let mut analysis = analyzer::analyze(&target, cargo_toml.as_deref(), assets_dir.as_deref())
                .context("Failed to analyze binary")?;

            if let Some(tc) = &tauri_conf {
                tauri_adapter::enrich(tc, &mut analysis);
            }

            let content = match format.to_lowercase().as_str() {
                "json" => export::to_json(&analysis)?,
                "prometheus" => export::to_prometheus(&analysis),
                "markdown" | "md" => export::to_markdown(&analysis),
                "csv" => export::to_csv(&analysis)?,
                other => anyhow::bail!("Unknown export format '{}'. Supported: json, prometheus, markdown, csv", other),
            };

            if let Some(path) = output {
                std::fs::write(&path, &content)
                    .context(format!("Failed to write to {}", path.display()))?;
                println!("Exported to {}", path.display());
            } else {
                println!("{}", content);
            }
        }
        Commands::Trend {
            history,
            limit,
            markdown,
        } => {
            let entries = persistence::load(&history)
                .context("Failed to load history. Run `guardian analyze --save` first.")?;
            if entries.is_empty() {
                anyhow::bail!("No history entries found in {}", history.display());
            }
            let t = trend::Trend::from_entries(&entries, limit);
            if markdown {
                print!("{}", t.to_markdown());
            } else {
                t.print();
            }
        }
        Commands::Alerts {
            target,
            history,
            cargo_toml,
            assets_dir,
        } => {
            let analysis = analyzer::analyze(&target, cargo_toml.as_deref(), assets_dir.as_deref())
                .context("Failed to analyze binary")?;

            let history_entries = persistence::load(&history).unwrap_or_default();
            let alerts = alert::check(&analysis, &history_entries);
            if alerts.is_empty() {
                println!("{}", colored::Colorize::green("✓ No alerts"));
            } else {
                for a in &alerts {
                    eprintln!("{}", a);
                }
                if alerts.iter().any(|a| a.severity == alert::AlertSeverity::Critical) {
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
