use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Complete analysis of a binary or app bundle.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BinaryAnalysis {
    /// Total size in bytes
    pub total_size: u64,
    /// Per-section sizes (text, data, rodata, bss, debug, etc.)
    pub sections: Vec<SectionInfo>,
    /// Per-crate/feature estimated sizes from symbol names
    pub features: Vec<FeatureSize>,
    /// Top individual symbols by size
    pub top_symbols: Vec<SymbolInfo>,
    /// Size of debug info sections
    pub debug_info_size: u64,
    /// Size of WebView assets if detectable
    pub webview_bundle_size: u64,
    /// Size of native library code
    pub native_lib_size: u64,
    /// Asset breakdown (icons, images, fonts, etc.)
    pub assets: Vec<AssetInfo>,
    /// Timestamp of analysis
    pub analyzed_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SectionInfo {
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureSize {
    pub name: String,
    pub size_bytes: u64,
    pub symbol_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub size_bytes: u64,
    pub section: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AssetInfo {
    pub path: String,
    pub size_bytes: u64,
    pub category: AssetCategory,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AssetCategory {
    Icon,
    Image,
    Font,
    Javascript,
    Css,
    Html,
    Wasm,
    Other,
}

/// Analyze a binary or bundle directory.
pub fn analyze(
    target: &Path,
    cargo_toml_path: Option<&Path>,
    assets_dir: Option<&Path>,
) -> Result<BinaryAnalysis> {
    let mut analysis = BinaryAnalysis {
        analyzed_at: chrono::Utc::now().to_rfc3339(),
        ..Default::default()
    };

    if target.is_dir() {
        analyze_bundle(target, &mut analysis)?;
    } else if target.is_file() {
        analyze_binary(target, &mut analysis)?;
    } else {
        anyhow::bail!("Target {} does not exist", target.display());
    }

    // Analyze assets if directory provided or part of a bundle
    if let Some(dir) = assets_dir {
        analyze_assets(dir, &mut analysis)?;
    } else if target.is_dir() {
        // Try common asset locations inside Tauri bundles
        for candidate in &["src-tauri", "dist", "public", "assets", "resources"] {
            let candidate_path = target.join(candidate);
            if candidate_path.is_dir() {
                analyze_assets(&candidate_path, &mut analysis)?;
            }
        }
    }

    // Estimate feature sizes from symbols using Cargo.toml if available
    if let Some(ct) = cargo_toml_path {
        estimate_features_from_cargo(ct, &mut analysis);
    }

    Ok(analysis)
}

fn analyze_bundle(dir: &Path, analysis: &mut BinaryAnalysis) -> Result<()> {
    // Walk the directory tree and categorize files
    let mut total = 0u64;
    let mut webview_total = 0u64;
    let mut native_total = 0u64;

    for entry in walkdir(dir) {
        let path = Path::new(&entry);
        if !path.is_file() {
            continue;
        }
        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        total += size;

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "html" | "js" | "css" | "woff" | "woff2" | "ttf" | "eot" | "svg" => {
                webview_total += size;
            }
            "so" | "dylib" | "dll" | "a" | "lib" => {
                native_total += size;
            }
            _ => {}
        }
    }

    analysis.total_size = total;
    analysis.webview_bundle_size = webview_total;
    analysis.native_lib_size = native_total;

    Ok(())
}

fn analyze_binary(path: &Path, analysis: &mut BinaryAnalysis) -> Result<()> {
    let metadata = path.metadata()?;
    analysis.total_size = metadata.len();

    // Try to run `size` (from binutils) for section info
    if let Ok(output) = std::process::Command::new("size")
        .arg("-A")
        .arg(path)
        .output()
    {
        if output.status.success() {
            parse_size_output(&String::from_utf8_lossy(&output.stdout), analysis);
        }
    }

    // Try `nm` for symbol analysis
    if let Ok(output) = std::process::Command::new("nm")
        .arg("--size-sort")
        .arg("--print-size")
        .arg(path)
        .output()
    {
        if output.status.success() {
            parse_nm_output(&String::from_utf8_lossy(&output.stdout), analysis);
        }
    }

    // Try to use `objdump` or `readelf` for more details
    if let Ok(output) = std::process::Command::new("readelf")
        .arg("-S")
        .arg(path)
        .output()
    {
        if output.status.success() {
            parse_readelf_sections(&String::from_utf8_lossy(&output.stdout), analysis);
        }
    }

    Ok(())
}

fn parse_size_output(output: &str, analysis: &mut BinaryAnalysis) {
    for line in output.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            if let (Some(name), Ok(size)) = (
                parts.last().map(|s| s.to_string()),
                parts.first().unwrap_or(&"").parse::<u64>(),
            ) {
                if name != ".comment" && !name.starts_with('(') {
                    analysis.sections.push(SectionInfo {
                        name,
                        size_bytes: size,
                    });
                }
            }
        }
    }
}

fn parse_nm_output(output: &str, analysis: &mut BinaryAnalysis) {
    let mut symbols: Vec<SymbolInfo> = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            if let (Ok(_addr), Ok(size)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                let name = parts[3..].join("_");
                symbols.push(SymbolInfo {
                    name,
                    size_bytes: size,
                    section: String::new(),
                });
            }
        }
    }
    // Sort by size descending, keep top 50
    symbols.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    symbols.truncate(50);
    analysis.top_symbols = symbols;
}

fn parse_readelf_sections(output: &str, analysis: &mut BinaryAnalysis) {
    analysis.debug_info_size = 0;
    for line in output.lines() {
        let line = line.trim();
        // Look for debug sections
        if line.contains(".debug_") || line.contains("__debug_") {
            // Extract size from the readelf output
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Try to find the size field (usually one of the numeric columns)
            for part in &parts {
                if let Ok(size) = part.parse::<u64>() {
                    if size > 0 && size < analysis.total_size {
                        analysis.debug_info_size += size;
                        break;
                    }
                }
            }
        }
    }
}

fn analyze_assets(dir: &Path, analysis: &mut BinaryAnalysis) -> Result<()> {
    for entry in walkdir(dir) {
        let path = Path::new(&entry);
        if !path.is_file() {
            continue;
        }
        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let category = match ext.as_str() {
            "png" | "ico" | "icns" => AssetCategory::Icon,
            "jpg" | "jpeg" | "gif" | "webp" | "bmp" => AssetCategory::Image,
            "woff" | "woff2" | "ttf" | "eot" | "otf" => AssetCategory::Font,
            "js" | "mjs" | "cjs" => AssetCategory::Javascript,
            "css" => AssetCategory::Css,
            "html" | "htm" => AssetCategory::Html,
            "wasm" => AssetCategory::Wasm,
            _ => AssetCategory::Other,
        };

        analysis.assets.push(AssetInfo {
            path: path.to_string_lossy().to_string(),
            size_bytes: size,
            category,
        });
    }

    Ok(())
}

fn estimate_features_from_cargo(cargo_toml_path: &Path, analysis: &mut BinaryAnalysis) {
    if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
        if let Ok(doc) = content.parse::<toml::Value>() {
            // Extract dependency names and estimate their contribution
            if let Some(deps) = doc
                .get("dependencies")
                .and_then(|d| d.as_table())
                .or_else(|| {
                    doc.get("workspace")
                        .and_then(|w| w.get("dependencies"))
                        .and_then(|d| d.as_table())
                })
            {
                let sym_map = build_symbol_prefix_map(&analysis.top_symbols);
                for (name, _) in deps {
                    let matched_symbols: Vec<_> = sym_map
                        .iter()
                        .filter(|(sym_name, _)| symbol_matches_crate(sym_name, name))
                        .collect();

                    let total_size: u64 = matched_symbols.iter().map(|(_, s)| *s).sum();
                    if total_size > 0 {
                        analysis.features.push(FeatureSize {
                            name: name.clone(),
                            size_bytes: total_size,
                            symbol_count: matched_symbols.len(),
                        });
                    }
                }
            }
        }
    }
}

fn build_symbol_prefix_map(symbols: &[SymbolInfo]) -> Vec<(String, u64)> {
    symbols
        .iter()
        .map(|s| (s.name.clone(), s.size_bytes))
        .collect()
}

fn symbol_matches_crate(symbol: &str, crate_name: &str) -> bool {
    // Rust symbols often start with crate name
    let snake = crate_name.replace('-', "_");
    symbol.starts_with(&snake) || symbol.contains(&format!("{}::", snake))
}

/// Simple recursive directory walk that doesn't require an external crate.
fn walkdir(root: &Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path));
            } else {
                result.push(path);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn analyze_nonexistent_path_returns_error() {
        let result = analyze(Path::new("/nonexistent/path"), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn analyze_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let analysis = analyze(dir.path(), None, None).unwrap();
        assert_eq!(analysis.total_size, 0);
    }

    #[test]
    fn analyze_directory_with_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("app.js"), "console.log('hello')").unwrap();
        fs::write(dir.path().join("style.css"), "body { margin: 0; }").unwrap();
        let analysis = analyze(dir.path(), None, None).unwrap();
        assert!(analysis.total_size > 0);
        assert!(analysis.webview_bundle_size > 0);
    }

    #[test]
    fn asset_categorization() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("icon.png"), vec![0u8; 1024]).unwrap();
        fs::write(dir.path().join("app.wasm"), vec![0u8; 2048]).unwrap();
        let analysis = analyze(dir.path(), None, Some(dir.path())).unwrap();
        assert!(analysis.assets.iter().any(|a| matches!(a.category, AssetCategory::Icon)));
        assert!(analysis.assets.iter().any(|a| matches!(a.category, AssetCategory::Wasm)));
    }

    #[test]
    fn symbol_matches_crate_works() {
        assert!(symbol_matches_crate("serde_json::value::Value", "serde_json"));
        assert!(symbol_matches_crate("tokio_runtime", "tokio-runtime"));
        assert!(!symbol_matches_crate("serde_json::value::Value", "tokio"));
    }
}
