use anyhow::{Context, Result};
use object::{Object, ObjectSection, ObjectSymbol};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Complete analysis of a binary or app bundle.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct BinaryAnalysis {
    pub total_size: u64,
    pub sections: Vec<SectionInfo>,
    pub features: Vec<FeatureSize>,
    pub top_symbols: Vec<SymbolInfo>,
    pub debug_info_size: u64,
    pub webview_bundle_size: u64,
    pub native_lib_size: u64,
    pub assets: Vec<AssetInfo>,
    pub analyzed_at: String,
    pub binary_format: String,
    pub architecture: String,
    pub tauri_meta: Option<TauriMeta>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TauriMeta {
    pub app_name: String,
    pub tauri_version: Option<String>,
    pub plugins: Vec<String>,
    pub bundle_identifier: Option<String>,
    pub window_count: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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
        analyze_bundle(target, &mut analysis);
    } else if target.is_file() {
        analyze_binary_native(target, &mut analysis)
            .context("Failed to analyze binary with native parser")?;
    } else {
        anyhow::bail!("Target {} does not exist", target.display());
    }

    if let Some(dir) = assets_dir {
        analyze_assets(dir, &mut analysis);
    } else if target.is_dir() {
        for candidate in &["src-tauri", "dist", "public", "assets", "resources"] {
            let candidate_path = target.join(candidate);
            if candidate_path.is_dir() {
                analyze_assets(&candidate_path, &mut analysis);
            }
        }
    }

    if let Some(ct) = cargo_toml_path {
        estimate_features_from_cargo(ct, &mut analysis);
    }

    Ok(analysis)
}

fn analyze_bundle(dir: &Path, analysis: &mut BinaryAnalysis) {
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
}

/// Analyze a binary using the `object` crate (cross-platform, no external tools).
fn analyze_binary_native(path: &Path, analysis: &mut BinaryAnalysis) -> Result<()> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read binary {}", path.display()))?;
    analysis.total_size = data.len() as u64;

    // Try to detect format
    if object::read::pe::PeFile64::parse(&*data).is_ok() {
        analysis.binary_format = "pe".to_string();
    } else if object::read::elf::ElfFile64::<object::Endianness>::parse(&*data).is_ok() {
        analysis.binary_format = "elf".to_string();
    } else if object::read::macho::MachOFile64::<object::Endianness>::parse(&*data).is_ok() {
        analysis.binary_format = "macho".to_string();
    } else {
        analysis.binary_format = "unknown".to_string();
    }

    parse_object_sections(&data, analysis);
    Ok(())
}

fn parse_object_sections(data: &[u8], analysis: &mut BinaryAnalysis) {
    if let Ok(obj_file) = object::read::File::parse(data) {
        analysis.architecture = format!("{:?}", obj_file.architecture());

        analysis.debug_info_size = 0;
        for section in obj_file.sections() {
            let name = section.name().unwrap_or("<<unknown>>").to_string();
            let size = section.size();

            if size > 0 {
                analysis.sections.push(SectionInfo {
                    name: name.clone(),
                    size_bytes: size,
                });

                if name.starts_with(".debug_")
                    || name.starts_with("__debug_")
                    || name == ".zdebug_"
                {
                    analysis.debug_info_size += size;
                }
            }
        }

        let mut symbols: Vec<SymbolInfo> = Vec::new();
        for symbol in obj_file.symbols() {
            let size = symbol.size();
            if size == 0 {
                continue;
            }
            let name = symbol.name().unwrap_or("").to_string();
            if name.is_empty() {
                continue;
            }
            let section_idx = symbol.section_index();
            let section_name = section_idx
                .and_then(|idx| obj_file.section_by_index(idx).ok())
                .and_then(|s| s.name().ok())
                .unwrap_or("")
                .to_string();

            symbols.push(SymbolInfo {
                name,
                size_bytes: size,
                section: section_name,
            });
        }

        symbols.sort_by_key(|b| std::cmp::Reverse(b.size_bytes));
        symbols.truncate(50);
        analysis.top_symbols = symbols;
    }
}

fn analyze_assets(dir: &Path, analysis: &mut BinaryAnalysis) {
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
}

fn estimate_features_from_cargo(cargo_toml_path: &Path, analysis: &mut BinaryAnalysis) {
    if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
        if let Ok(doc) = content.parse::<toml::Value>() {
            if let Some(deps) = doc
                .get("dependencies")
                .and_then(|d| d.as_table())
                .or_else(|| {
                    doc.get("workspace")
                        .and_then(|w| w.get("dependencies"))
                        .and_then(|d| d.as_table())
                })
            {
                let sym_map: Vec<(String, u64)> = analysis
                    .top_symbols
                    .iter()
                    .map(|s| (s.name.clone(), s.size_bytes))
                    .collect();

                for (name, _) in deps {
                    let matched: Vec<_> = sym_map
                        .iter()
                        .filter(|(sym_name, _)| symbol_matches_crate(sym_name, name))
                        .collect();

                    let total_size: u64 = matched.iter().map(|(_, s)| *s).sum();
                    if total_size > 0 {
                        analysis.features.push(FeatureSize {
                            name: name.clone(),
                            size_bytes: total_size,
                            symbol_count: matched.len(),
                        });
                    }
                }
            }
        }
    }
}

fn symbol_matches_crate(symbol: &str, crate_name: &str) -> bool {
    let snake = crate_name.replace('-', "_");
    symbol.starts_with(&snake) || symbol.contains(&format!("{}::", snake))
}

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
        assert!(analysis
            .assets
            .iter()
            .any(|a| matches!(a.category, AssetCategory::Icon)));
        assert!(analysis
            .assets
            .iter()
            .any(|a| matches!(a.category, AssetCategory::Wasm)));
    }

    #[test]
    fn symbol_matches_crate_works() {
        assert!(symbol_matches_crate(
            "serde_json::value::Value",
            "serde_json"
        ));
        assert!(symbol_matches_crate("tokio_runtime", "tokio-runtime"));
        assert!(!symbol_matches_crate(
            "serde_json::value::Value",
            "tokio"
        ));
    }

    #[test]
    fn analyze_self_binary() {
        let self_path = std::env::current_exe().unwrap();
        let analysis = analyze(&self_path, None, None).unwrap();
        assert!(analysis.total_size > 0);
        #[cfg(target_os = "linux")]
        assert_eq!(analysis.binary_format, "elf");
        assert!(!analysis.sections.is_empty());
    }
}
