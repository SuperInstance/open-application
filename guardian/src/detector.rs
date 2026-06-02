use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use crate::analyzer::{AssetCategory, BinaryAnalysis};
use crate::budget::format_size;

/// A bloat finding with human-readable explanation.
#[derive(Debug)]
pub struct BloatFinding {
    pub severity: Severity,
    pub category: String,
    pub description: String,
    pub suggestion: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
}

/// Detect bloat patterns in a binary analysis.
pub fn detect(
    analysis: &BinaryAnalysis,
    cargo_toml_path: Option<&Path>,
) -> Result<Vec<BloatFinding>> {
    let mut findings = Vec::new();

    detect_oversized_features(analysis, &mut findings);
    detect_debug_info(analysis, &mut findings);
    detect_asset_bloat(analysis, &mut findings);
    detect_unused_deps(analysis, cargo_toml_path, &mut findings);
    detect_webview_bloat(analysis, &mut findings);

    findings.sort_by(|a, b| b.severity.cmp(&a.severity));
    Ok(findings)
}

fn detect_oversized_features(analysis: &BinaryAnalysis, findings: &mut Vec<BloatFinding>) {
    const BLOAT_THRESHOLD: u64 = 500 * 1024; // 500KB

    for feature in &analysis.features {
        if feature.size_bytes > BLOAT_THRESHOLD {
            findings.push(BloatFinding {
                severity: if feature.size_bytes > 2 * 1024 * 1024 {
                    Severity::High
                } else {
                    Severity::Medium
                },
                category: "oversized_feature".into(),
                description: format!(
                    "Feature '{}' adds {} across {} symbols",
                    feature.name,
                    format_size(feature.size_bytes),
                    feature.symbol_count
                ),
                suggestion: format!(
                    "Consider whether {} is worth its size cost, or if a lighter alternative exists.",
                    feature.name
                ),
            });
        }
    }
}

fn detect_debug_info(analysis: &BinaryAnalysis, findings: &mut Vec<BloatFinding>) {
    if analysis.total_size == 0 {
        return;
    }
    let ratio = analysis.debug_info_size as f64 / analysis.total_size as f64;

    if ratio > 0.15 {
        findings.push(BloatFinding {
            severity: Severity::High,
            category: "debug_info".into(),
            description: format!(
                "Debug info is {:.0}% of binary ({} of {})",
                ratio * 100.0,
                format_size(analysis.debug_info_size),
                format_size(analysis.total_size)
            ),
            suggestion: "Strip debug symbols from release builds with `strip = true` in Cargo.toml profile settings.".into(),
        });
    } else if ratio > 0.05 {
        findings.push(BloatFinding {
            severity: Severity::Low,
            category: "debug_info".into(),
            description: format!(
                "Debug info is {:.0}% of binary ({} of {})",
                ratio * 100.0,
                format_size(analysis.debug_info_size),
                format_size(analysis.total_size)
            ),
            suggestion: "Consider stripping debug symbols for smaller release builds.".into(),
        });
    }
}

fn detect_asset_bloat(analysis: &BinaryAnalysis, findings: &mut Vec<BloatFinding>) {
    // Group assets by category
    let mut category_sizes: std::collections::HashMap<&AssetCategory, (u64, usize)> =
        std::collections::HashMap::new();
    for asset in &analysis.assets {
        let entry = category_sizes.entry(&asset.category).or_insert((0, 0));
        entry.0 += asset.size_bytes;
        entry.1 += 1;
    }

    // Check for oversized icon collections
    let icon_total = category_sizes
        .get(&AssetCategory::Icon)
        .map(|(s, c)| (*s, *c))
        .unwrap_or((0, 0));

    if icon_total.0 > 1024 * 1024 {
        findings.push(BloatFinding {
            severity: Severity::Medium,
            category: "icon_bloat".into(),
            description: format!(
                "Icons total {} across {} files",
                format_size(icon_total.0),
                icon_total.1
            ),
            suggestion: "Use a single vector icon (SVG) or optimize PNG icons with tools like optipng or zopflipng.".into(),
        });
    }

    // Check for large individual assets
    for asset in &analysis.assets {
        if asset.size_bytes > 512 * 1024 {
            findings.push(BloatFinding {
                severity: if asset.size_bytes > 1024 * 1024 {
                    Severity::Medium
                } else {
                    Severity::Low
                },
                category: "large_asset".into(),
                description: format!(
                    "'{}' is {}",
                    Path::new(&asset.path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown"),
                    format_size(asset.size_bytes)
                ),
                suggestion: "Optimize or compress this asset. Consider lazy-loading if it's not needed immediately.".into(),
            });
        }
    }
}

fn detect_unused_deps(
    analysis: &BinaryAnalysis,
    cargo_toml_path: Option<&Path>,
    findings: &mut Vec<BloatFinding>,
) {
    let Some(ct_path) = cargo_toml_path else { return };
    let Ok(content) = std::fs::read_to_string(ct_path) else {
        return;
    };
    let Ok(doc) = content.parse::<toml::Value>() else {
        return;
    };

    let Some(deps) = doc.get("dependencies").and_then(|d| d.as_table()) else {
        return;
    };

    let used_crates: HashSet<String> = analysis
        .features
        .iter()
        .map(|f| f.name.replace('-', "_"))
        .collect();

    let mut unused = Vec::new();
    for (name, val) in deps {
        // Skip optional dependencies — they may be behind feature flags
        if val
            .as_table()
            .map(|t| t.get("optional").and_then(|o| o.as_bool()).unwrap_or(false))
            .unwrap_or(false)
        {
            continue;
        }

        let snake = name.replace('-', "_");
        if !used_crates.contains(&snake) {
            unused.push(name.clone());
        }
    }

    if !unused.is_empty() {
        findings.push(BloatFinding {
            severity: if unused.len() > 3 {
                Severity::Medium
            } else {
                Severity::Low
            },
            category: "unused_deps".into(),
            description: format!(
                "{} dependencies in Cargo.toml have no detected symbols in the binary: {}",
                unused.len(),
                unused.join(", ")
            ),
            suggestion:
                "These may be unused or conditionally compiled. Run `cargo machete` to confirm and remove them.".into(),
        });
    }
}

fn detect_webview_bloat(analysis: &BinaryAnalysis, findings: &mut Vec<BloatFinding>) {
    if analysis.total_size == 0 {
        return;
    }
    let ratio = analysis.webview_bundle_size as f64 / analysis.total_size as f64;

    if ratio > 0.50 {
        findings.push(BloatFinding {
            severity: Severity::High,
            category: "webview_bloat".into(),
            description: format!(
                "WebView assets are {:.0}% of total app size ({} of {})",
                ratio * 100.0,
                format_size(analysis.webview_bundle_size),
                format_size(analysis.total_size)
            ),
            suggestion: "Consider code-splitting, tree-shaking, or moving to a native UI for heavy views. Use a bundler optimizer like terser or esbuild minification.".into(),
        });
    } else if ratio > 0.35 {
        findings.push(BloatFinding {
            severity: Severity::Low,
            category: "webview_bloat".into(),
            description: format!(
                "WebView assets are {:.0}% of total app size ({} of {})",
                ratio * 100.0,
                format_size(analysis.webview_bundle_size),
                format_size(analysis.total_size)
            ),
            suggestion: "Review your WebView assets for optimization opportunities.".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::*;

    #[test]
    fn detects_oversized_features() {
        let analysis = BinaryAnalysis {
            total_size: 10 * 1024 * 1024,
            features: vec![FeatureSize {
                name: "serde_json".into(),
                size_bytes: 800 * 1024,
                symbol_count: 120,
            }],
            ..Default::default()
        };
        let findings = detect(&analysis, None).unwrap();
        assert!(findings.iter().any(|f| f.category == "oversized_feature"));
    }

    #[test]
    fn no_bloat_when_clean() {
        let analysis = BinaryAnalysis {
            total_size: 5 * 1024 * 1024,
            ..Default::default()
        };
        let findings = detect(&analysis, None).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn detects_debug_info_ratio() {
        let analysis = BinaryAnalysis {
            total_size: 10 * 1024 * 1024,
            debug_info_size: 2 * 1024 * 1024, // 20%
            ..Default::default()
        };
        let findings = detect(&analysis, None).unwrap();
        assert!(findings.iter().any(|f| f.category == "debug_info" && f.severity == Severity::High));
    }
}
