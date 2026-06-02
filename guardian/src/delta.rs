use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::analyzer::BinaryAnalysis;
use crate::budget::format_size;

/// The computed delta between two binary analyses.
#[derive(Debug, Serialize, Deserialize)]
pub struct Delta {
    pub before_size: u64,
    pub after_size: u64,
    pub total_delta_bytes: i64,
    pub total_delta_pct: f64,
    pub section_deltas: Vec<SectionDelta>,
    pub feature_deltas: Vec<FeatureDelta>,
    pub verdict: Verdict,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SectionDelta {
    pub name: String,
    pub before: u64,
    pub after: u64,
    pub delta_bytes: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FeatureDelta {
    pub name: String,
    pub before: u64,
    pub after: u64,
    pub delta_bytes: i64,
    pub appeared: bool,
    pub disappeared: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum Verdict {
    /// Significant shrinkage — good
    Shrunk,
    /// Within 1% — neutral
    Stable,
    /// Grew 1-5% — caution
    GrewSlightly,
    /// Grew >5% — warning
    Grew,
}

impl Delta {
    pub fn compute(before: &BinaryAnalysis, after: &BinaryAnalysis) -> Self {
        let total_delta_bytes = after.total_size as i64 - before.total_size as i64;
        let total_delta_pct = if before.total_size > 0 {
            total_delta_bytes as f64 / before.total_size as f64 * 100.0
        } else {
            0.0
        };

        let verdict = if total_delta_pct < -1.0 {
            Verdict::Shrunk
        } else if total_delta_pct <= 1.0 {
            Verdict::Stable
        } else if total_delta_pct <= 5.0 {
            Verdict::GrewSlightly
        } else {
            Verdict::Grew
        };

        // Section deltas
        let mut section_deltas = Vec::new();
        let mut before_sections: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
        let mut after_sections: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();

        for s in &before.sections {
            before_sections.insert(&s.name, s.size_bytes);
        }
        for s in &after.sections {
            after_sections.insert(&s.name, s.size_bytes);
        }

        let mut all_section_names: std::collections::BTreeSet<&str> =
            std::collections::BTreeSet::new();
        for name in before_sections.keys() {
            all_section_names.insert(name);
        }
        for name in after_sections.keys() {
            all_section_names.insert(name);
        }

        for name in all_section_names {
            let b = before_sections.get(name).copied().unwrap_or(0);
            let a = after_sections.get(name).copied().unwrap_or(0);
            let delta = a as i64 - b as i64;
            // Only include sections that changed
            if delta != 0 {
                section_deltas.push(SectionDelta {
                    name: name.to_string(),
                    before: b,
                    after: a,
                    delta_bytes: delta,
                });
            }
        }

        section_deltas.sort_by(|a, b| b.delta_bytes.abs().cmp(&a.delta_bytes.abs()));

        // Feature deltas
        let mut feature_deltas = Vec::new();
        let before_features: std::collections::HashMap<&str, u64> = before
            .features
            .iter()
            .map(|f| (f.name.as_str(), f.size_bytes))
            .collect();
        let after_features: std::collections::HashMap<&str, u64> = after
            .features
            .iter()
            .map(|f| (f.name.as_str(), f.size_bytes))
            .collect();

        let mut all_feature_names: std::collections::BTreeSet<&str> =
            std::collections::BTreeSet::new();
        for name in before_features.keys() {
            all_feature_names.insert(name);
        }
        for name in after_features.keys() {
            all_feature_names.insert(name);
        }

        for name in all_feature_names {
            let b = before_features.get(name).copied().unwrap_or(0);
            let a = after_features.get(name).copied().unwrap_or(0);
            let delta = a as i64 - b as i64;
            feature_deltas.push(FeatureDelta {
                name: name.to_string(),
                before: b,
                after: a,
                delta_bytes: delta,
                appeared: b == 0 && a > 0,
                disappeared: b > 0 && a == 0,
            });
        }

        feature_deltas.sort_by(|a, b| b.delta_bytes.abs().cmp(&a.delta_bytes.abs()));

        Delta {
            before_size: before.total_size,
            after_size: after.total_size,
            total_delta_bytes,
            total_delta_pct,
            section_deltas,
            feature_deltas,
            verdict,
        }
    }

    pub fn print_report(&self) {
        println!();
        println!(
            "{} {}",
            "📊 Build Delta Report".bold().cyan(),
            "═══════════════════════════════════".dimmed()
        );
        println!();

        let delta_str = if self.total_delta_bytes > 0 {
            format!(
                "+{} ({:+.1}%)",
                format_size(self.total_delta_bytes as u64),
                self.total_delta_pct
            )
            .red()
            .to_string()
        } else if self.total_delta_bytes < 0 {
            format!(
                "{} ({:+.1}%)",
                format_size(self.total_delta_bytes.unsigned_abs()),
                self.total_delta_pct
            )
            .green()
            .to_string()
        } else {
            "no change".dimmed().to_string()
        };

        println!(
            "  {} → {} ({})",
            format_size(self.before_size),
            format_size(self.after_size),
            delta_str
        );

        let verdict_str = match self.verdict {
            Verdict::Shrunk => "🌱 Shrunk".green().to_string(),
            Verdict::Stable => "⚖ Stable".white().to_string(),
            Verdict::GrewSlightly => "📈 Grew slightly".yellow().to_string(),
            Verdict::Grew => "🚨 Significant growth".red().bold().to_string(),
        };
        println!("  Verdict: {}", verdict_str);
        println!();

        // Section changes
        if !self.section_deltas.is_empty() {
            println!("  {}", "Section Changes".bold());
            for sec in self.section_deltas.iter().take(10) {
                let arrow = if sec.delta_bytes > 0 {
                    "↑".red().to_string()
                } else {
                    "↓".green().to_string()
                };
                println!(
                    "    {} {:<25} {} → {} ({})",
                    arrow,
                    sec.name,
                    format_size(sec.before),
                    format_size(sec.after),
                    format_delta(sec.delta_bytes)
                );
            }
            println!();
        }

        // Feature changes
        if !self.feature_deltas.is_empty() {
            println!("  {}", "Feature Changes".bold());
            for feat in self.feature_deltas.iter().take(10) {
                let tag = if feat.appeared {
                    " [NEW]".yellow().to_string()
                } else if feat.disappeared {
                    " [REMOVED]".green().to_string()
                } else {
                    String::new()
                };
                let arrow = if feat.delta_bytes > 0 {
                    "↑".red().to_string()
                } else {
                    "↓".green().to_string()
                };
                println!(
                    "    {} {:<25} {} → {} ({}{})",
                    arrow,
                    feat.name,
                    format_size(feat.before),
                    format_size(feat.after),
                    format_delta(feat.delta_bytes),
                    tag
                );
            }
            println!();
        }

        println!(
            "  {}",
            "═══════════════════════════════════".dimmed()
        );
    }
}

fn format_delta(delta: i64) -> String {
    if delta > 0 {
        format!("+{}", format_size(delta as u64)).red().to_string()
    } else if delta < 0 {
        format!("-{}", format_size(delta.unsigned_abs()))
            .green()
            .to_string()
    } else {
        "—".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::*;

    #[test]
    fn delta_detects_growth() {
        let before = BinaryAnalysis {
            total_size: 10 * 1024 * 1024,
            ..Default::default()
        };
        let after = BinaryAnalysis {
            total_size: 11 * 1024 * 1024,
            ..Default::default()
        };
        let delta = Delta::compute(&before, &after);
        assert_eq!(delta.verdict, Verdict::Grew);
        assert!(delta.total_delta_bytes > 0);
    }

    #[test]
    fn delta_detects_shrinkage() {
        let before = BinaryAnalysis {
            total_size: 10 * 1024 * 1024,
            ..Default::default()
        };
        let after = BinaryAnalysis {
            total_size: 8 * 1024 * 1024,
            ..Default::default()
        };
        let delta = Delta::compute(&before, &after);
        assert_eq!(delta.verdict, Verdict::Shrunk);
    }

    #[test]
    fn delta_detects_stable() {
        let before = BinaryAnalysis {
            total_size: 10 * 1024 * 1024,
            ..Default::default()
        };
        let after = BinaryAnalysis {
            total_size: 10 * 1024 * 1024 + 50 * 1024, // ~0.5% change
            ..Default::default()
        };
        let delta = Delta::compute(&before, &after);
        assert_eq!(delta.verdict, Verdict::Stable);
    }

    #[test]
    fn delta_tracks_feature_changes() {
        let before = BinaryAnalysis {
            total_size: 10 * 1024 * 1024,
            features: vec![FeatureSize {
                name: "serde".into(),
                size_bytes: 500 * 1024,
                symbol_count: 100,
            }],
            ..Default::default()
        };
        let after = BinaryAnalysis {
            total_size: 10 * 1024 * 1024,
            features: vec![
                FeatureSize {
                    name: "serde".into(),
                    size_bytes: 600 * 1024,
                    symbol_count: 120,
                },
                FeatureSize {
                    name: "tokio".into(),
                    size_bytes: 800 * 1024,
                    symbol_count: 200,
                },
            ],
            ..Default::default()
        };
        let delta = Delta::compute(&before, &after);
        assert!(delta.feature_deltas.iter().any(|f| f.name == "tokio" && f.appeared));
        assert!(delta.feature_deltas.iter().any(|f| f.name == "serde" && f.delta_bytes == 100 * 1024));
    }
}
