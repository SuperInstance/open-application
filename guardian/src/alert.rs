use std::fmt;

use crate::analyzer::BinaryAnalysis;
use crate::budget::format_size;
use crate::persistence::HistoryEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertSeverity {
    Warning,
    Critical,
}

pub struct Alert {
    pub severity: AlertSeverity,
    pub category: String,
    pub message: String,
}

impl fmt::Display for Alert {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use colored::Colorize;
        let icon = match self.severity {
            AlertSeverity::Warning => "⚠️".to_string(),
            AlertSeverity::Critical => "🚨".to_string(),
        };
        let label = match self.severity {
            AlertSeverity::Warning => "WARN".yellow().to_string(),
            AlertSeverity::Critical => "CRIT".red().bold().to_string(),
        };
        write!(f, "{} [{}] {}: {}", icon, label, self.category, self.message)
    }
}

/// Check for alert conditions.
pub fn check(analysis: &BinaryAnalysis, history: &[HistoryEntry]) -> Vec<Alert> {
    let mut alerts = Vec::new();

    check_growth_spike(analysis, history, &mut alerts);
    check_large_new_dep(analysis, history, &mut alerts);
    check_absolute_size(analysis, &mut alerts);
    check_debug_ratio(analysis, &mut alerts);

    alerts
}

/// Alert if binary grew > 10% since last build.
fn check_growth_spike(
    analysis: &BinaryAnalysis,
    history: &[HistoryEntry],
    alerts: &mut Vec<Alert>,
) {
    let Some(last) = history.last() else {
        return;
    };
    if last.total_size == 0 {
        return;
    }
    let growth_pct = (analysis.total_size as f64 - last.total_size as f64) / last.total_size as f64 * 100.0;

    if growth_pct > 10.0 {
        alerts.push(Alert {
            severity: AlertSeverity::Critical,
            category: "growth_spike".into(),
            message: format!(
                "Binary grew {:.1}% since last build ({} → {})",
                growth_pct,
                format_size(last.total_size),
                format_size(analysis.total_size)
            ),
        });
    } else if growth_pct > 5.0 {
        alerts.push(Alert {
            severity: AlertSeverity::Warning,
            category: "growth_spike".into(),
            message: format!(
                "Binary grew {:.1}% since last build ({} → {})",
                growth_pct,
                format_size(last.total_size),
                format_size(analysis.total_size)
            ),
        });
    }
}

/// Alert if a new dependency added 500KB+.
fn check_large_new_dep(
    analysis: &BinaryAnalysis,
    history: &[HistoryEntry],
    alerts: &mut Vec<Alert>,
) {
    let Some(last) = history.last() else {
        return;
    };

    let prev_names: std::collections::HashSet<&str> = last
        .top_features
        .iter()
        .map(|(n, _)| n.as_str())
        .collect();

    for feature in &analysis.features {
        if !prev_names.contains(feature.name.as_str()) && feature.size_bytes >= 500 * 1024 {
            alerts.push(Alert {
                severity: AlertSeverity::Warning,
                category: "large_new_dep".into(),
                message: format!(
                    "New dependency '{}' adds {}",
                    feature.name,
                    format_size(feature.size_bytes)
                ),
            });
        }
    }
}

/// Alert on absolute size thresholds.
fn check_absolute_size(analysis: &BinaryAnalysis, alerts: &mut Vec<Alert>) {
    // 50MB is a big Tauri app
    if analysis.total_size > 50 * 1024 * 1024 {
        alerts.push(Alert {
            severity: AlertSeverity::Critical,
            category: "absolute_size".into(),
            message: format!(
                "Binary is {} — exceeds 50MB threshold",
                format_size(analysis.total_size)
            ),
        });
    } else if analysis.total_size > 30 * 1024 * 1024 {
        alerts.push(Alert {
            severity: AlertSeverity::Warning,
            category: "absolute_size".into(),
            message: format!(
                "Binary is {} — approaching 50MB threshold",
                format_size(analysis.total_size)
            ),
        });
    }
}

/// Alert if debug info is too large relative to binary.
fn check_debug_ratio(analysis: &BinaryAnalysis, alerts: &mut Vec<Alert>) {
    if analysis.total_size == 0 {
        return;
    }
    let ratio = analysis.debug_info_size as f64 / analysis.total_size as f64;
    if ratio > 0.20 {
        alerts.push(Alert {
            severity: AlertSeverity::Critical,
            category: "debug_info".into(),
            message: format!(
                "Debug info is {:.0}% of binary ({}). Strip with `strip = true` in Cargo.toml.",
                ratio * 100.0,
                format_size(analysis.debug_info_size)
            ),
        });
    } else if ratio > 0.10 {
        alerts.push(Alert {
            severity: AlertSeverity::Warning,
            category: "debug_info".into(),
            message: format!(
                "Debug info is {:.0}% of binary ({})",
                ratio * 100.0,
                format_size(analysis.debug_info_size)
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::*;

    #[test]
    fn no_alerts_on_first_build() {
        let analysis = BinaryAnalysis {
            total_size: 5 * 1024 * 1024,
            ..Default::default()
        };
        let alerts = check(&analysis, &[]);
        assert!(alerts.is_empty());
    }

    #[test]
    fn growth_spike_alert() {
        let history = vec![HistoryEntry {
            timestamp: "2024-01-01T00:00:00Z".into(),
            total_size: 5 * 1024 * 1024,
            webview_bundle_size: 0,
            native_lib_size: 0,
            debug_info_size: 0,
            binary_format: "elf".into(),
            architecture: "x86_64".into(),
            section_count: 0,
            feature_count: 0,
            top_features: vec![],
        }];
        let analysis = BinaryAnalysis {
            total_size: 6 * 1024 * 1024, // 20% growth
            ..Default::default()
        };
        let alerts = check(&analysis, &history);
        assert!(alerts.iter().any(|a| a.category == "growth_spike" && a.severity == AlertSeverity::Critical));
    }

    #[test]
    fn large_new_dep_alert() {
        let history = vec![HistoryEntry {
            timestamp: "2024-01-01T00:00:00Z".into(),
            total_size: 5 * 1024 * 1024,
            top_features: vec![("serde".into(), 500 * 1024)],
            webview_bundle_size: 0,
            native_lib_size: 0,
            debug_info_size: 0,
            binary_format: "elf".into(),
            architecture: "x86_64".into(),
            section_count: 0,
            feature_count: 0,
        }];
        let analysis = BinaryAnalysis {
            total_size: 6 * 1024 * 1024,
            features: vec![
                FeatureSize {
                    name: "serde".into(),
                    size_bytes: 500 * 1024,
                    symbol_count: 50,
                },
                FeatureSize {
                    name: "tokio".into(),
                    size_bytes: 800 * 1024, // new dep > 500KB
                    symbol_count: 200,
                },
            ],
            ..Default::default()
        };
        let alerts = check(&analysis, &history);
        assert!(alerts.iter().any(|a| a.category == "large_new_dep"));
    }

    #[test]
    fn absolute_size_alert() {
        let analysis = BinaryAnalysis {
            total_size: 55 * 1024 * 1024,
            ..Default::default()
        };
        let alerts = check(&analysis, &[]);
        assert!(alerts.iter().any(|a| a.category == "absolute_size" && a.severity == AlertSeverity::Critical));
    }
}
