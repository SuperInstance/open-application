use anyhow::Result;

use crate::analyzer::BinaryAnalysis;
use crate::budget::format_size;

/// Export analysis as pretty-printed JSON.
pub fn to_json(analysis: &BinaryAnalysis) -> Result<String> {
    Ok(serde_json::to_string_pretty(analysis)?)
}

/// Export analysis in Prometheus exposition format.
pub fn to_prometheus(analysis: &BinaryAnalysis) -> String {
    let mut lines = Vec::new();

    lines.push("# HELP guardian_binary_size_bytes Total binary size in bytes".to_string());
    lines.push("# TYPE guardian_binary_size_bytes gauge".to_string());
    lines.push(format!(
        "guardian_binary_size_bytes {}",
        analysis.total_size
    ));

    lines.push(String::new());
    lines.push("# HELP guardian_section_size_bytes Size of binary sections".to_string());
    lines.push("# TYPE guardian_section_size_bytes gauge".to_string());
    for section in &analysis.sections {
        let name = sanitize_label(&section.name);
        lines.push(format!(
            "guardian_section_size_bytes{{section=\"{}\"}} {}",
            name, section.size_bytes
        ));
    }

    lines.push(String::new());
    lines.push("# HELP guardian_feature_size_bytes Estimated size per crate/feature".to_string());
    lines.push("# TYPE guardian_feature_size_bytes gauge".to_string());
    for feature in &analysis.features {
        let name = sanitize_label(&feature.name);
        lines.push(format!(
            "guardian_feature_size_bytes{{feature=\"{}\"}} {}",
            name, feature.size_bytes
        ));
    }

    lines.push(String::new());
    lines.push("# HELP guardian_webview_bundle_size_bytes WebView asset bundle size".to_string());
    lines.push("# TYPE guardian_webview_bundle_size_bytes gauge".to_string());
    lines.push(format!(
        "guardian_webview_bundle_size_bytes {}",
        analysis.webview_bundle_size
    ));

    lines.push(String::new());
    lines.push("# HELP guardian_debug_info_size_bytes Debug info section size".to_string());
    lines.push("# TYPE guardian_debug_info_size_bytes gauge".to_string());
    lines.push(format!(
        "guardian_debug_info_size_bytes {}",
        analysis.debug_info_size
    ));

    lines.push(String::new());
    lines.push("# HELP guardian_native_lib_size_bytes Native library size".to_string());
    lines.push("# TYPE guardian_native_lib_size_bytes gauge".to_string());
    lines.push(format!(
        "guardian_native_lib_size_bytes {}",
        analysis.native_lib_size
    ));

    lines.join("\n")
}

/// Export analysis as a Markdown table.
pub fn to_markdown(analysis: &BinaryAnalysis) -> String {
    let mut lines = Vec::new();

    lines.push("# App Size Guardian Report".to_string());
    lines.push(String::new());
    lines.push(format!("**Total Size:** {}", format_size(analysis.total_size)));
    lines.push(format!("**Format:** {} | **Arch:** {}", analysis.binary_format, analysis.architecture));
    lines.push(format!(
        "**WebView:** {} ({:.1}%) | **Native:** {} ({:.1}%) | **Debug:** {} ({:.1}%)",
        format_size(analysis.webview_bundle_size),
        if analysis.total_size > 0 { analysis.webview_bundle_size as f64 / analysis.total_size as f64 * 100.0 } else { 0.0 },
        format_size(analysis.native_lib_size),
        if analysis.total_size > 0 { analysis.native_lib_size as f64 / analysis.total_size as f64 * 100.0 } else { 0.0 },
        format_size(analysis.debug_info_size),
        if analysis.total_size > 0 { analysis.debug_info_size as f64 / analysis.total_size as f64 * 100.0 } else { 0.0 },
    ));
    lines.push(String::new());

    if !analysis.sections.is_empty() {
        lines.push("## Sections".to_string());
        lines.push(String::new());
        lines.push("| Section | Size |".to_string());
        lines.push("|---------|------|".to_string());
        for s in &analysis.sections {
            lines.push(format!("| {} | {} |", s.name, format_size(s.size_bytes)));
        }
        lines.push(String::new());
    }

    if !analysis.features.is_empty() {
        lines.push("## Features by Size".to_string());
        lines.push(String::new());
        lines.push("| Feature | Size | Symbols |".to_string());
        lines.push("|---------|------|---------|".to_string());
        for f in &analysis.features {
            lines.push(format!("| {} | {} | {} |", f.name, format_size(f.size_bytes), f.symbol_count));
        }
        lines.push(String::new());
    }

    if !analysis.assets.is_empty() {
        let mut sorted = analysis.assets.clone();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.size_bytes));
        lines.push("## Top Assets".to_string());
        lines.push(String::new());
        lines.push("| Asset | Size | Category |".to_string());
        lines.push("|-------|------|----------|".to_string());
        for a in sorted.iter().take(20) {
            let name = std::path::Path::new(&a.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            lines.push(format!("| {} | {} | {:?} |", name, format_size(a.size_bytes), a.category));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

/// Export analysis as CSV.
pub fn to_csv(analysis: &BinaryAnalysis) -> Result<String> {
    let mut wtr = csv::Writer::from_writer(Vec::new());

    // Section rows
    for s in &analysis.sections {
        wtr.write_record(["section", &s.name, &s.size_bytes.to_string()])?;
    }

    // Feature rows
    for f in &analysis.features {
        wtr.write_record(["feature", &f.name, &f.size_bytes.to_string()])?;
    }

    // Summary rows
    wtr.write_record(["summary", "total_size", &analysis.total_size.to_string()])?;
    wtr.write_record(["summary", "webview_bundle_size", &analysis.webview_bundle_size.to_string()])?;
    wtr.write_record(["summary", "native_lib_size", &analysis.native_lib_size.to_string()])?;
    wtr.write_record(["summary", "debug_info_size", &analysis.debug_info_size.to_string()])?;

    let bytes = wtr.into_inner()?;
    Ok(String::from_utf8(bytes)?)
}

fn sanitize_label(s: &str) -> String {
    s.replace(['\\', '"', ' ', '-'], "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::*;

    #[test]
    fn json_export_works() {
        let analysis = BinaryAnalysis {
            total_size: 1024 * 1024,
            ..Default::default()
        };
        let json = to_json(&analysis).unwrap();
        assert!(json.contains("1048576"));
    }

    #[test]
    fn prometheus_export_format() {
        let analysis = BinaryAnalysis {
            total_size: 2048,
            sections: vec![SectionInfo {
                name: ".text".into(),
                size_bytes: 1024,
            }],
            ..Default::default()
        };
        let prom = to_prometheus(&analysis);
        assert!(prom.contains("guardian_binary_size_bytes 2048"));
        assert!(prom.contains("section=\".text\""));
    }

    #[test]
    fn markdown_export_has_tables() {
        let analysis = BinaryAnalysis {
            total_size: 4096,
            features: vec![FeatureSize {
                name: "serde".into(),
                size_bytes: 1024,
                symbol_count: 10,
            }],
            ..Default::default()
        };
        let md = to_markdown(&analysis);
        assert!(md.contains("## Features by Size"));
        assert!(md.contains("| serde |"));
    }

    #[test]
    fn csv_export_is_valid() {
        let analysis = BinaryAnalysis {
            total_size: 4096,
            sections: vec![SectionInfo {
                name: ".text".into(),
                size_bytes: 2048,
            }],
            ..Default::default()
        };
        let csv = to_csv(&analysis).unwrap();
        assert!(csv.contains("section,.text,2048"));
    }
}
