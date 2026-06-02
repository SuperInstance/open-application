use std::path::Path;

use crate::analyzer::{BinaryAnalysis, TauriMeta};

/// Enrich analysis with Tauri-specific metadata from tauri.conf.json.
pub fn enrich(tauri_conf_path: &Path, analysis: &mut BinaryAnalysis) {
    let content = match std::fs::read_to_string(tauri_conf_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let conf: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    let app_name = conf["productName"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let tauri_version = conf["version"].as_str().map(|s| s.to_string());

    let bundle_id = conf["identifier"].as_str().map(|s| s.to_string());

    // Extract plugin list from plugins field
    let plugins: Vec<String> = conf
        .get("plugins")
        .and_then(|p| p.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default();

    // Count windows
    let window_count = conf
        .get("app")
        .and_then(|a| a.get("windows"))
        .and_then(|w| w.as_array())
        .map(|w| w.len())
        .unwrap_or(0);

    // Cross-reference features with detected plugins
    let _plugin_features: Vec<&str> = plugins
        .iter()
        .filter(|p| {
            let snake = p.replace('-', "_");
            analysis
                .features
                .iter()
                .any(|f| f.name.contains(&snake) || snake.contains(&f.name))
        })
        .map(|p| p.as_str())
        .collect();

    analysis.tauri_meta = Some(TauriMeta {
        app_name,
        tauri_version,
        plugins,
        bundle_identifier: bundle_id,
        window_count,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::BinaryAnalysis;
    use std::fs;

    #[test]
    fn enrich_parses_tauri_conf() {
        let dir = tempfile::tempdir().unwrap();
        let conf_path = dir.path().join("tauri.conf.json");
        fs::write(
            &conf_path,
            r#"{
                "productName": "TestApp",
                "version": "2.0.0",
                "identifier": "com.test.app",
                "plugins": {
                    "sql": {},
                    "fs": {}
                },
                "app": {
                    "windows": [
                        {"title": "main"},
                        {"title": "about"}
                    ]
                }
            }"#,
        )
        .unwrap();

        let mut analysis = BinaryAnalysis::default();
        enrich(&conf_path, &mut analysis);

        let meta = analysis.tauri_meta.unwrap();
        assert_eq!(meta.app_name, "TestApp");
        assert_eq!(meta.tauri_version.as_deref(), Some("2.0.0"));
        assert_eq!(meta.window_count, 2);
        assert!(meta.plugins.contains(&"sql".to_string()));
        assert!(meta.plugins.contains(&"fs".to_string()));
    }

    #[test]
    fn enrich_handles_missing_file() {
        let mut analysis = BinaryAnalysis::default();
        enrich(Path::new("/nonexistent/tauri.conf.json"), &mut analysis);
        assert!(analysis.tauri_meta.is_none());
    }
}
