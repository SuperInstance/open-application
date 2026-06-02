use crate::analyzer::BinaryAnalysis;
use crate::budget::format_size;
use crate::detector;

/// Human-readable conservation report.
pub struct Report {
    total_size: u64,
    webview_size: u64,
    webview_pct: f64,
    native_size: u64,
    native_pct: f64,
    debug_size: u64,
    debug_pct: f64,
    top_assets: Vec<AssetLine>,
    section_breakdown: Vec<(String, u64, f64)>,
    bloat_findings: Vec<detector::BloatFinding>,
    top_features: Vec<FeatureLine>,
}

struct AssetLine {
    name: String,
    size: u64,
    pct: f64,
    category: String,
}

struct FeatureLine {
    name: String,
    size: u64,
    symbol_count: usize,
}

impl Report {
    pub fn from_analysis(analysis: &BinaryAnalysis) -> Self {
        let total = analysis.total_size.max(1);

        // Asset breakdown: top 10 by size
        let mut assets_sorted = analysis.assets.clone();
        assets_sorted.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));

        let top_assets: Vec<AssetLine> = assets_sorted
            .iter()
            .take(10)
            .map(|a| {
                let name = std::path::Path::new(&a.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                AssetLine {
                    name,
                    size: a.size_bytes,
                    pct: a.size_bytes as f64 / total as f64 * 100.0,
                    category: format!("{:?}", a.category).to_lowercase(),
                }
            })
            .collect();

        // Section breakdown
        let section_breakdown: Vec<(String, u64, f64)> = analysis
            .sections
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    s.size_bytes,
                    s.size_bytes as f64 / total as f64 * 100.0,
                )
            })
            .collect();

        // Top features
        let mut features_sorted = analysis.features.clone();
        features_sorted.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        let top_features: Vec<FeatureLine> = features_sorted
            .iter()
            .take(10)
            .map(|f| FeatureLine {
                name: f.name.clone(),
                size: f.size_bytes,
                symbol_count: f.symbol_count,
            })
            .collect();

        let bloat_findings = detector::detect(analysis, None).unwrap_or_default();

        Report {
            total_size: analysis.total_size,
            webview_size: analysis.webview_bundle_size,
            webview_pct: analysis.webview_bundle_size as f64 / total as f64 * 100.0,
            native_size: analysis.native_lib_size,
            native_pct: analysis.native_lib_size as f64 / total as f64 * 100.0,
            debug_size: analysis.debug_info_size,
            debug_pct: analysis.debug_info_size as f64 / total as f64 * 100.0,
            top_assets,
            section_breakdown,
            bloat_findings,
            top_features,
        }
    }

    pub fn print(&self) {
        use colored::Colorize;

        println!();
        println!(
            "{} {}",
            "📐 App Size Guardian Report".bold().cyan(),
            "═══════════════════════════════════".dimmed()
        );
        println!();

        // Overview
        println!(
            "  Your app is {}",
            format_size(self.total_size).bold().white()
        );
        println!();

        // Composition
        println!("  {}", "Composition".bold());
        println!(
            "    WebView assets  {} ({:.1}%)",
            format_size(self.webview_size),
            self.webview_pct
        );
        println!(
            "    Native code     {} ({:.1}%)",
            format_size(self.native_size),
            self.native_pct
        );
        if self.debug_size > 0 {
            println!(
                "    Debug info      {} ({:.1}%)",
                format_size(self.debug_size),
                self.debug_pct
            );
        }
        println!();

        // Top assets
        if !self.top_assets.is_empty() {
            println!("  {}", "Largest Assets".bold());
            for asset in &self.top_assets {
                println!(
                    "    {} — {} ({:.1}%, {})",
                    asset.name.yellow(),
                    format_size(asset.size),
                    asset.pct,
                    asset.category
                );
            }
            println!();
        }

        // Top features by size
        if !self.top_features.is_empty() {
            println!("  {}", "Features by Size".bold());
            for feat in self.top_features.iter().take(5) {
                println!(
                    "    {:<25} {} ({} symbols)",
                    feat.name,
                    format_size(feat.size),
                    feat.symbol_count
                );
            }
            println!();
        }

        // Section breakdown
        if !self.section_breakdown.is_empty() {
            println!("  {}", "Binary Sections".bold());
            for (name, size, pct) in self.section_breakdown.iter().take(8) {
                println!("    {:<25} {} ({:.1}%)", name, format_size(*size), pct);
            }
            println!();
        }

        // Bloat findings
        if !self.bloat_findings.is_empty() {
            println!("  {}", "⚠ Bloat Detected".bold().red());
            println!();
            for finding in &self.bloat_findings {
                let icon = match finding.severity {
                    detector::Severity::High => "🔴".to_string(),
                    detector::Severity::Medium => "🟡".to_string(),
                    detector::Severity::Low => "🟢".to_string(),
                };
                println!("    {} {}", icon, finding.description);
                println!(
                    "       {} {}",
                    "→".dimmed(),
                    finding.suggestion.dimmed()
                );
            }
            println!();
        } else {
            println!("  {}", "✓ No bloat detected".green().bold());
            println!();
        }

        println!(
            "  {}",
            "═══════════════════════════════════".dimmed()
        );
    }
}

/// Build a natural-language conservation report paragraph.
pub fn paragraph(analysis: &BinaryAnalysis) -> String {
    let total = analysis.total_size.max(1);
    let total_str = format_size(analysis.total_size);
    let webview_str = format_size(analysis.webview_bundle_size);
    let webview_pct = analysis.webview_bundle_size as f64 / total as f64 * 100.0;

    let mut lines = vec![format!(
        "Your app is {}. WebView assets = {} ({:.0}%).",
        total_str, webview_str, webview_pct
    )];

    // Top 3 assets
    let mut assets_sorted = analysis.assets.clone();
    assets_sorted.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    let top3: Vec<_> = assets_sorted.iter().take(3).collect();
    if top3.len() >= 2 {
        let total_top3: u64 = top3.iter().map(|a| a.size_bytes).sum();
        let top3_names: Vec<String> = top3
            .iter()
            .map(|a| {
                std::path::Path::new(&a.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            })
            .collect();
        lines.push(format!(
            "{} account for {}.",
            top3_names.join(", "),
            format_size(total_top3)
        ));
    }

    // Bloat findings
    let findings = detector::detect(analysis, None).unwrap_or_default();
    for f in findings.iter().take(3) {
        lines.push(f.description.clone());
    }

    lines.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::*;

    #[test]
    fn report_does_not_panic_on_empty_analysis() {
        let analysis = BinaryAnalysis::default();
        let _ = Report::from_analysis(&analysis);
    }

    #[test]
    fn paragraph_mentions_total_size() {
        let analysis = BinaryAnalysis {
            total_size: 12 * 1024 * 1024 + 300 * 1024,
            webview_bundle_size: 4 * 1024 * 1024 + 100 * 1024,
            ..Default::default()
        };
        let text = paragraph(&analysis);
        assert!(text.contains("12.3MB"));
        assert!(text.contains("4.1MB"));
    }
}
