use colored::Colorize;

use crate::budget::format_size;
use crate::persistence::HistoryEntry;

/// Trend analysis across historical builds.
pub struct Trend {
    pub entries: Vec<HistoryEntry>,
}

impl Trend {
    pub fn from_entries(all_entries: &[HistoryEntry], limit: usize) -> Self {
        let start = all_entries.len().saturating_sub(limit);
        Trend {
            entries: all_entries[start..].to_vec(),
        }
    }

    /// Print a human-readable trend report.
    pub fn print(&self) {
        if self.entries.is_empty() {
            println!("No history entries.");
            return;
        }

        println!();
        println!(
            "{} {}",
            "📈 Size Trend".bold().cyan(),
            "═══════════════════════════════════".dimmed()
        );
        println!();

        // Overall summary
        let first = &self.entries[0];
        let last = self.entries.last().unwrap();
        let total_delta = last.total_size as i64 - first.total_size as i64;
        let builds = self.entries.len();

        println!("  {} builds tracked", builds);
        println!(
            "  {} → {} ({})",
            format_size(first.total_size),
            format_size(last.total_size),
            if total_delta > 0 {
                format!("+{}", format_size(total_delta as u64)).red().to_string()
            } else if total_delta < 0 {
                format!("-{}", format_size(total_delta.unsigned_abs()))
                    .green()
                    .to_string()
            } else {
                "no change".dimmed().to_string()
            }
        );

        // Per-build delta
        if builds > 1 {
            let pct = if first.total_size > 0 {
                total_delta as f64 / first.total_size as f64 * 100.0
            } else {
                0.0
            };
            println!("  Overall trend: {:+.1}%", pct);
        }
        println!();

        // Build-by-build
        println!("  {}", "Build History".bold());
        for (i, entry) in self.entries.iter().enumerate() {
            let delta = if i > 0 {
                let prev = self.entries[i - 1].total_size as i64;
                let d = entry.total_size as i64 - prev;
                if d > 0 {
                    format!("(+{})", format_size(d as u64)).red().to_string()
                } else if d < 0 {
                    format!("(-{})", format_size(d.unsigned_abs()))
                        .green()
                        .to_string()
                } else {
                    "(-)".dimmed().to_string()
                }
            } else {
                String::new()
            };

            // Truncate timestamp to date
            let ts = entry
                .timestamp
                .split('T')
                .next()
                .unwrap_or(&entry.timestamp);
            println!(
                "    {:>3}. {} {} {}",
                i + 1,
                ts.white(),
                format_size(entry.total_size).bold(),
                delta
            );
        }

        // Top contributors to growth
        if builds > 1 {
            println!();
            println!("  {}", "Top Contributors to Growth".bold());
            let first_features: std::collections::HashMap<&str, u64> = first
                .top_features
                .iter()
                .map(|(n, s)| (n.as_str(), *s))
                .collect();
            let last_features: std::collections::HashMap<&str, u64> = last
                .top_features
                .iter()
                .map(|(n, s)| (n.as_str(), *s))
                .collect();

            let mut contributors: Vec<(String, i64)> = Vec::new();
            let mut all_names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
            for n in first_features.keys() {
                all_names.insert(n);
            }
            for n in last_features.keys() {
                all_names.insert(n);
            }

            for name in all_names {
                let b = first_features.get(name).copied().unwrap_or(0);
                let a = last_features.get(name).copied().unwrap_or(0);
                let d = a as i64 - b as i64;
                if d != 0 {
                    contributors.push((name.to_string(), d));
                }
            }

            contributors.sort_by_key(|b| std::cmp::Reverse(b.1.abs()));
            for (name, delta) in contributors.iter().take(5) {
                let arrow = if *delta > 0 { "↑".red() } else { "↓".green() };
                println!(
                    "    {} {:<25} {}",
                    arrow,
                    name,
                    if *delta > 0 {
                        format!("+{}", format_size(*delta as u64)).red().to_string()
                    } else {
                        format!("-{}", format_size(delta.unsigned_abs()))
                            .green()
                            .to_string()
                    }
                );
            }
        }

        println!();
        println!(
            "  {}",
            "═══════════════════════════════════".dimmed()
        );
    }

    /// Export as markdown table.
    pub fn to_markdown(&self) -> String {
        let mut lines = vec!["# Size Trend".to_string(), String::new()];

        lines.push("| # | Date | Size | Delta |".to_string());
        lines.push("|---|------|------|-------|".to_string());

        for (i, entry) in self.entries.iter().enumerate() {
            let ts = entry.timestamp.split('T').next().unwrap_or(&entry.timestamp);
            let delta = if i > 0 {
                let prev = self.entries[i - 1].total_size as i64;
                let d = entry.total_size as i64 - prev;
                if d > 0 {
                    format!("+{}", format_size(d as u64))
                } else if d < 0 {
                    format!("-{}", format_size(d.unsigned_abs()))
                } else {
                    "—".to_string()
                }
            } else {
                "—".to_string()
            };
            lines.push(format!(
                "| {} | {} | {} | {} |",
                i + 1,
                ts,
                format_size(entry.total_size),
                delta
            ));
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(n: usize, size: u64) -> HistoryEntry {
        HistoryEntry {
            timestamp: format!("2024-01-{:02}T00:00:00Z", n + 1),
            total_size: size,
            webview_bundle_size: 0,
            native_lib_size: 0,
            debug_info_size: 0,
            binary_format: "elf".into(),
            architecture: "x86_64".into(),
            section_count: 0,
            feature_count: 0,
            top_features: vec![],
        }
    }

    #[test]
    fn trend_limits_entries() {
        let entries: Vec<HistoryEntry> = (0..20).map(|i| make_entry(i, (i as u64 + 1) * 1024 * 1024)).collect();
        let trend = Trend::from_entries(&entries, 5);
        assert_eq!(trend.entries.len(), 5);
    }

    #[test]
    fn trend_markdown_has_table() {
        let entries = vec![make_entry(0, 1024 * 1024), make_entry(1, 2 * 1024 * 1024)];
        let trend = Trend::from_entries(&entries, 10);
        let md = trend.to_markdown();
        assert!(md.contains("| # | Date | Size | Delta |"));
    }
}
