use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration loaded from a TOML file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BudgetConfig {
    /// Maximum total app size in bytes (default: 15 MB)
    pub max_total_bytes: u64,
    /// Maximum size for any single feature/crate in bytes (default: 2 MB)
    pub max_per_feature_bytes: u64,
    /// Maximum WebView (HTML/CSS/JS/assets) bundle size in bytes (default: 5 MB)
    pub max_webview_bundle_bytes: u64,
    /// Maximum native library size in bytes (default: 8 MB)
    pub max_native_lib_bytes: u64,
    /// Features that are exempt from per-feature limits
    #[serde(default)]
    pub exempt_features: Vec<String>,
    /// Maximum debug info ratio (0.0–1.0 of total binary)
    pub max_debug_info_ratio: f64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_total_bytes: 15 * 1024 * 1024,         // 15 MB
            max_per_feature_bytes: 2 * 1024 * 1024,     // 2 MB
            max_webview_bundle_bytes: 5 * 1024 * 1024,  // 5 MB
            max_native_lib_bytes: 8 * 1024 * 1024,      // 8 MB
            exempt_features: vec!["tauri".into(), "webview".into()],
            max_debug_info_ratio: 0.10,
        }
    }
}

impl BudgetConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}

/// The resolved budget with enforcement logic.
pub struct SizeBudget {
    config: BudgetConfig,
}

impl SizeBudget {
    pub fn from_config(config: &BudgetConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn check(&self, analysis: &crate::analyzer::BinaryAnalysis) -> Vec<BudgetViolation> {
        let mut violations = Vec::new();

        // Total size
        if analysis.total_size > self.config.max_total_bytes {
            violations.push(BudgetViolation {
                category: "total".into(),
                actual: analysis.total_size,
                limit: self.config.max_total_bytes,
            });
        }

        // Per-feature (crate) sizes
        for feature in &analysis.features {
            if self.config.exempt_features.contains(&feature.name) {
                continue;
            }
            if feature.size_bytes > self.config.max_per_feature_bytes {
                violations.push(BudgetViolation {
                    category: format!("feature:{}", feature.name),
                    actual: feature.size_bytes,
                    limit: self.config.max_per_feature_bytes,
                });
            }
        }

        // WebView bundle
        if analysis.webview_bundle_size > self.config.max_webview_bundle_bytes {
            violations.push(BudgetViolation {
                category: "webview_bundle".into(),
                actual: analysis.webview_bundle_size,
                limit: self.config.max_webview_bundle_bytes,
            });
        }

        // Native lib
        if analysis.native_lib_size > self.config.max_native_lib_bytes {
            violations.push(BudgetViolation {
                category: "native_lib".into(),
                actual: analysis.native_lib_size,
                limit: self.config.max_native_lib_bytes,
            });
        }

        // Debug info ratio
        if analysis.total_size > 0 {
            let ratio = analysis.debug_info_size as f64 / analysis.total_size as f64;
            if ratio > self.config.max_debug_info_ratio {
                violations.push(BudgetViolation {
                    category: "debug_info_ratio".into(),
                    actual: (ratio * 1000.0) as u64, // store as per-mille for display
                    limit: (self.config.max_debug_info_ratio * 1000.0) as u64,
                });
            }
        }

        violations
    }
}

#[derive(Debug)]
pub struct BudgetViolation {
    pub category: String,
    pub actual: u64,
    pub limit: u64,
}

impl std::fmt::Display for BudgetViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use colored::Colorize;
        let _actual_fmt = format_size(self.actual);
        let limit_fmt = format_size(self.limit);
        write!(
            f,
            "{} {} exceeds limit of {} ({} over)",
            "✗".red().bold(),
            self.category.yellow(),
            limit_fmt,
            format_size(self.actual.saturating_sub(self.limit)).red()
        )
    }
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_budget_is_reasonable() {
        let config = BudgetConfig::default();
        assert_eq!(config.max_total_bytes, 15 * 1024 * 1024);
        assert_eq!(config.max_per_feature_bytes, 2 * 1024 * 1024);
    }

    #[test]
    fn format_size_handles_units() {
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1024), "1.0KB");
        assert_eq!(format_size(1024 * 1024), "1.0MB");
    }

    #[test]
    fn check_total_violation() {
        let config = BudgetConfig::default();
        let budget = SizeBudget::from_config(&config);
        let analysis = crate::analyzer::BinaryAnalysis {
            total_size: 20 * 1024 * 1024,
            ..Default::default()
        };
        let violations = budget.check(&analysis);
        assert!(violations.iter().any(|v| v.category == "total"));
    }
}
