//! Capability Loader — Loads CAPABILITY.toml files and generates Tauri commands.
//!
//! This module reads CAPABILITY.toml files from our Rust crates (math, spectral,
//! fleet, etc.) and generates Tauri command definitions for each capability. This
//! is how our Rust math crates become UI features — each capability becomes a
//! callable command from the frontend.
//!
//! # CAPABILITY.toml Format
//!
//! ```toml
//! [capability]
//! name = "spectral_search"
//! description = "Search using spectral eigenvalue similarity"
//! version = "0.1.0"
//!
//! [capability.inputs]
//! query = { type = "String", description = "Search query vector" }
//! top_k = { type = "usize", description = "Number of results", default = 10 }
//!
//! [capability.outputs]
//! results = { type = "Vec<SearchResult>", description = "Ranked search results" }
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A single input parameter for a capability.
#[derive(Debug, Clone)]
pub struct CapabilityInput {
    pub type_name: String,
    pub description: String,
    pub default_value: Option<String>,
}

/// A single output field from a capability.
#[derive(Debug, Clone)]
pub struct CapabilityOutput {
    pub type_name: String,
    pub description: String,
}

/// Parsed CAPABILITY.toml representing a single capability.
#[derive(Debug, Clone)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub version: String,
    pub inputs: HashMap<String, CapabilityInput>,
    pub outputs: HashMap<String, CapabilityOutput>,
}

/// A generated Tauri command derived from a capability.
#[derive(Debug, Clone)]
pub struct TauriCommand {
    pub command_name: String,
    pub capability_name: String,
    pub parameters: Vec<(String, String)>, // (name, type)
    pub return_type: String,
    pub description: String,
    /// Generated Rust source code for the command handler.
    pub source_code: String,
}

/// Errors that can occur during capability loading.
#[derive(Debug)]
pub enum CapabilityError {
    IoError(String),
    ParseError(String),
    ValidationError(String),
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityError::IoError(msg) => write!(f, "IO error: {}", msg),
            CapabilityError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            CapabilityError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

/// The capability loader that scans directories for CAPABILITY.toml files.
pub struct CapabilityLoader {
    /// Loaded capabilities indexed by name.
    capabilities: HashMap<String, Capability>,
    /// Generated Tauri commands.
    commands: Vec<TauriCommand>,
}

impl CapabilityLoader {
    /// Create a new CapabilityLoader.
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
            commands: Vec::new(),
        }
    }

    /// Load all CAPABILITY.toml files from a directory.
    ///
    /// Recursively scans the directory for files named CAPABILITY.toml,
    /// parses each one, and generates corresponding Tauri commands.
    pub fn load_capabilities(&mut self, dir: &str) -> Result<Vec<TauriCommand>, CapabilityError> {
        let path = Path::new(dir);
        if !path.exists() {
            return Err(CapabilityError::IoError(format!(
                "Directory does not exist: {}",
                dir
            )));
        }

        let mut found = Vec::new();
        self.scan_directory(path, &mut found)?;
        Ok(found)
    }

    fn scan_directory(
        &mut self,
        dir: &Path,
        results: &mut Vec<TauriCommand>,
    ) -> Result<(), CapabilityError> {
        let entries = fs::read_dir(dir)
            .map_err(|e| CapabilityError::IoError(format!("Cannot read {:?}: {}", dir, e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| CapabilityError::IoError(e.to_string()))?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_directory(&path, results)?;
            } else if path
                .file_name()
                .map(|n| n == "CAPABILITY.toml")
                .unwrap_or(false)
            {
                let content = fs::read_to_string(&path).map_err(|e| {
                    CapabilityError::IoError(format!("Cannot read {:?}: {}", path, e))
                })?;
                let capability = self.parse_capability(&content)?;
                let command = self.generate_command(&capability);
                results.push(command.clone());
                self.capabilities
                    .insert(capability.name.clone(), capability);
                self.commands.push(command);
            }
        }

        Ok(())
    }

    /// Parse a CAPABILITY.toml file content into a Capability struct.
    ///
    /// Simple TOML-like parser for the CAPABILITY.toml format.
    pub fn parse_capability(&self, content: &str) -> Result<Capability, CapabilityError> {
        let mut name = String::new();
        let mut description = String::new();
        let mut version = String::new();
        let mut inputs: HashMap<String, CapabilityInput> = HashMap::new();
        let mut outputs: HashMap<String, CapabilityOutput> = HashMap::new();

        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Section headers
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].trim().to_string();
                continue;
            }

            // Key-value pairs
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().trim_matches('"').to_string();

                match current_section.as_str() {
                    "capability" => match key.as_str() {
                        "name" => name = value,
                        "description" => description = value,
                        "version" => version = value,
                        _ => {}
                    },
                    _ if current_section.starts_with("capability.inputs") => {
                        // Simplified: treat as type definition
                        inputs.insert(
                            key,
                            CapabilityInput {
                                type_name: value,
                                description: String::new(),
                                default_value: None,
                            },
                        );
                    }
                    _ if current_section.starts_with("capability.outputs") => {
                        outputs.insert(
                            key,
                            CapabilityOutput {
                                type_name: value,
                                description: String::new(),
                            },
                        );
                    }
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            return Err(CapabilityError::ValidationError(
                "Capability name is required".into(),
            ));
        }

        Ok(Capability {
            name,
            description,
            version,
            inputs,
            outputs,
        })
    }

    /// Parse a CAPABILITY.toml from a raw string with extended inline-table format.
    ///
    /// Handles: `key = { type = "String", description = "...", default = "..." }`
    pub fn parse_capability_extended(&self, content: &str) -> Result<Capability, CapabilityError> {
        let mut cap = self.parse_capability(content)?;

        // Re-parse for inline table syntax in inputs/outputs
        let mut current_section = String::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].trim().to_string();
                continue;
            }

            if line.contains("={") || line.contains("= {") {
                if let Some(eq_pos) = line.find('=') {
                    let key = line[..eq_pos].trim().to_string();
                    let value_part = line[eq_pos + 1..].trim();

                    let type_val = extract_field(value_part, "type");
                    let desc_val = extract_field(value_part, "description");
                    let default_val = extract_field(value_part, "default");

                    if current_section.contains("inputs") {
                        if let Some(type_name) = type_val {
                            cap.inputs.insert(
                                key,
                                CapabilityInput {
                                    type_name,
                                    description: desc_val.unwrap_or_default(),
                                    default_value: default_val,
                                },
                            );
                        }
                    } else if current_section.contains("outputs") {
                        if let Some(type_name) = type_val {
                            cap.outputs.insert(
                                key,
                                CapabilityOutput {
                                    type_name,
                                    description: desc_val.unwrap_or_default(),
                                },
                            );
                        }
                    }
                }
            }
        }

        Ok(cap)
    }

    /// Generate a Tauri command from a capability definition.
    pub fn generate_command(&self, capability: &Capability) -> TauriCommand {
        let command_name = format!("cmd_{}", capability.name);

        let params: Vec<(String, String)> = capability
            .inputs
            .iter()
            .map(|(name, input)| (name.clone(), input.type_name.clone()))
            .collect();

        let return_type = if capability.outputs.is_empty() {
            "()".to_string()
        } else if capability.outputs.len() == 1 {
            capability
                .outputs
                .values()
                .next()
                .map(|o| o.type_name.clone())
                .unwrap_or_else(|| "()".to_string())
        } else {
            // Multiple outputs -> tuple
            let types: Vec<&str> = capability
                .outputs
                .values()
                .map(|o| o.type_name.as_str())
                .collect();
            format!("({})", types.join(", "))
        };

        let param_str: Vec<String> = params
            .iter()
            .map(|(n, t)| format!("{}: {}", n, t))
            .collect();
        let param_list = param_str.join(", ");

        let source_code = format!(
            r#"#[tauri::command]
fn {command_name}({param_list}) -> Result<{return_type}, String> {{
    // Auto-generated from CAPABILITY.toml: {cap_name}
    // Implementation provided by agent_runtime
    todo!("Implement {} capability")
}}"#,
            command_name = command_name,
            param_list = param_list,
            return_type = return_type,
            cap_name = capability.name,
        );

        TauriCommand {
            command_name,
            capability_name: capability.name.clone(),
            parameters: params,
            return_type,
            description: capability.description.clone(),
            source_code,
        }
    }

    /// Get all loaded capabilities.
    pub fn capabilities(&self) -> &HashMap<String, Capability> {
        &self.capabilities
    }

    /// Get all generated commands.
    pub fn commands(&self) -> &[TauriCommand] {
        &self.commands
    }
}

/// Extract a named field from an inline table string like `{ type = "String", ... }`.
fn extract_field(table: &str, field: &str) -> Option<String> {
    let pattern = format!("{}=\"", field);
    if let Some(start) = table.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = table[val_start..].find('"') {
            return Some(table[val_start..val_start + end].to_string());
        }
    }
    let pattern_space = format!("{} = \"", field);
    if let Some(start) = table.find(&pattern_space) {
        let val_start = start + pattern_space.len();
        if let Some(end) = table[val_start..].find('"') {
            return Some(table[val_start..val_start + end].to_string());
        }
    }
    None
}

impl Default for CapabilityLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("cap_test_{}_{}", std::process::id(), id));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
        fn path_str(&self) -> &str {
            self.path.to_str().unwrap()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_parse_simple_capability() {
        let loader = CapabilityLoader::new();
        let toml = r#"
[capability]
name = "spectral_search"
description = "Search using spectral eigenvalue similarity"
version = "0.1.0"
"#;
        let cap = loader.parse_capability(toml).unwrap();
        assert_eq!(cap.name, "spectral_search");
        assert_eq!(cap.description, "Search using spectral eigenvalue similarity");
        assert_eq!(cap.version, "0.1.0");
    }

    #[test]
    fn test_parse_capability_missing_name() {
        let loader = CapabilityLoader::new();
        let toml = r#"
[capability]
description = "No name"
"#;
        let result = loader.parse_capability(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_extended_capability() {
        let loader = CapabilityLoader::new();
        let toml = r#"
[capability]
name = "eigen_search"
description = "Eigenvalue-based search"
version = "1.0.0"

[capability.inputs]
query = { type = "String", description = "Query vector" }
top_k = { type = "usize", description = "Result count", default = "10" }

[capability.outputs]
results = { type = "Vec<SearchResult>", description = "Ranked results" }
"#;
        let cap = loader.parse_capability_extended(toml).unwrap();
        assert_eq!(cap.name, "eigen_search");
        assert!(cap.inputs.contains_key("query"));
        assert!(cap.inputs.contains_key("top_k"));
        assert_eq!(cap.inputs["query"].type_name, "String");
        assert_eq!(cap.inputs["top_k"].default_value, Some("10".to_string()));
        assert!(cap.outputs.contains_key("results"));
    }

    #[test]
    fn test_generate_command() {
        let loader = CapabilityLoader::new();
        let cap = Capability {
            name: "search".into(),
            description: "Search for things".into(),
            version: "0.1.0".into(),
            inputs: {
                let mut m = HashMap::new();
                m.insert(
                    "query".into(),
                    CapabilityInput {
                        type_name: "String".into(),
                        description: "Search query".into(),
                        default_value: None,
                    },
                );
                m
            },
            outputs: {
                let mut m = HashMap::new();
                m.insert(
                    "results".into(),
                    CapabilityOutput {
                        type_name: "Vec<String>".into(),
                        description: "Results".into(),
                    },
                );
                m
            },
        };
        let cmd = loader.generate_command(&cap);
        assert_eq!(cmd.command_name, "cmd_search");
        assert_eq!(cmd.return_type, "Vec<String>");
        assert!(cmd.source_code.contains("#[tauri::command]"));
        assert!(cmd.source_code.contains("query: String"));
    }

    #[test]
    fn test_generate_command_no_outputs() {
        let loader = CapabilityLoader::new();
        let cap = Capability {
            name: "ping".into(),
            description: "Health check".into(),
            version: "0.1.0".into(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        };
        let cmd = loader.generate_command(&cap);
        assert_eq!(cmd.return_type, "()");
    }

    #[test]
    fn test_generate_command_multiple_outputs() {
        let loader = CapabilityLoader::new();
        let cap = Capability {
            name: "analyze".into(),
            description: "Analyze data".into(),
            version: "0.1.0".into(),
            inputs: HashMap::new(),
            outputs: {
                let mut m = HashMap::new();
                m.insert(
                    "score".into(),
                    CapabilityOutput {
                        type_name: "f64".into(),
                        description: "Score".into(),
                    },
                );
                m.insert(
                    "label".into(),
                    CapabilityOutput {
                        type_name: "String".into(),
                        description: "Label".into(),
                    },
                );
                m
            },
        };
        let cmd = loader.generate_command(&cap);
        assert!(cmd.return_type.starts_with('('));
        assert!(cmd.return_type.contains("f64"));
        assert!(cmd.return_type.contains("String"));
    }

    #[test]
    fn test_load_from_nonexistent_directory() {
        let mut loader = CapabilityLoader::new();
        let result = loader.load_capabilities("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_directory_with_capability() {
        let tmpdir = TempDir::new();
        let cap_content = r#"
[capability]
name = "test_cap"
description = "A test capability"
version = "0.1.0"
"#;
        let cap_path = tmpdir.path.join("CAPABILITY.toml");
        let mut f = fs::File::create(&cap_path).unwrap();
        f.write_all(cap_content.as_bytes()).unwrap();

        let mut loader = CapabilityLoader::new();
        let commands = loader.load_capabilities(tmpdir.path_str()).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].capability_name, "test_cap");
    }

    #[test]
    fn test_load_empty_directory() {
        let tmpdir = TempDir::new();
        let mut loader = CapabilityLoader::new();
        let commands = loader
            .load_capabilities(tmpdir.path_str())
            .unwrap();
        assert!(commands.is_empty());
    }

    #[test]
    fn test_extract_field() {
        let table = r#"{ type = "String", description = "A field", default = "hello" }"#;
        assert_eq!(extract_field(table, "type"), Some("String".to_string()));
        assert_eq!(
            extract_field(table, "description"),
            Some("A field".to_string())
        );
        assert_eq!(
            extract_field(table, "default"),
            Some("hello".to_string())
        );
        assert_eq!(extract_field(table, "missing"), None);
    }
}
