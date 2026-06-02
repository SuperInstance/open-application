use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::analyzer::BinaryAnalysis;

/// A single history entry (one analysis snapshot).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub total_size: u64,
    pub webview_bundle_size: u64,
    pub native_lib_size: u64,
    pub debug_info_size: u64,
    pub binary_format: String,
    pub architecture: String,
    pub section_count: usize,
    pub feature_count: usize,
    pub top_features: Vec<(String, u64)>,
}

/// The full history file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

impl From<&BinaryAnalysis> for HistoryEntry {
    fn from(a: &BinaryAnalysis) -> Self {
        let mut top_features: Vec<(String, u64)> = a
            .features
            .iter()
            .map(|f| (f.name.clone(), f.size_bytes))
            .collect();
        top_features.sort_by_key(|b| std::cmp::Reverse(b.1));
        top_features.truncate(10);

        HistoryEntry {
            timestamp: a.analyzed_at.clone(),
            total_size: a.total_size,
            webview_bundle_size: a.webview_bundle_size,
            native_lib_size: a.native_lib_size,
            debug_info_size: a.debug_info_size,
            binary_format: a.binary_format.clone(),
            architecture: a.architecture.clone(),
            section_count: a.sections.len(),
            feature_count: a.features.len(),
            top_features,
        }
    }
}

/// Load history from a JSON file.
pub fn load(path: &Path) -> Result<Vec<HistoryEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let history: History = serde_json::from_str(&content)?;
    Ok(history.entries)
}

/// Append an analysis to the history file.
pub fn append(path: &Path, analysis: &BinaryAnalysis) -> Result<()> {
    let mut entries = load(path).unwrap_or_default();
    entries.push(HistoryEntry::from(analysis));

    let history = History { entries };
    let json = serde_json::to_string_pretty(&history)?;
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::*;
    use std::fs;

    #[test]
    fn roundtrip_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");

        let analysis = BinaryAnalysis {
            total_size: 1024 * 1024,
            analyzed_at: "2024-01-01T00:00:00Z".to_string(),
            binary_format: "elf".to_string(),
            architecture: "x86_64".to_string(),
            features: vec![FeatureSize {
                name: "serde".into(),
                size_bytes: 500 * 1024,
                symbol_count: 50,
            }],
            ..Default::default()
        };

        append(&path, &analysis).unwrap();
        let entries = load(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].total_size, 1024 * 1024);
        assert_eq!(entries[0].top_features.len(), 1);
    }

    #[test]
    fn multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");

        for i in 0..5 {
            let analysis = BinaryAnalysis {
                total_size: (i + 1) * 1024 * 1024,
                analyzed_at: format!("2024-01-0{}T00:00:00Z", i + 1),
                ..Default::default()
            };
            append(&path, &analysis).unwrap();
        }

        let entries = load(&path).unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn load_missing_file() {
        let entries = load(Path::new("/nonexistent/history.json")).unwrap();
        assert!(entries.is_empty());
    }
}
