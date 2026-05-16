//! Configuration loader supporting multiple formats

use super::Config;
use anyhow::{Context, Result};
use std::path::Path;

/// Load configuration from a file
///
/// Supports multiple formats:
/// - `.yaml`, `.yml`: YAML format
/// - `.json`: JSON format
/// - `.toml`: TOML format
/// - `.proxy`: Caddy-like DSL format (10x simpler)
pub fn load_config(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let config: Config = match path.extension().and_then(|s| s.to_str()) {
        Some("yaml") | Some("yml") => serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML config: {}", e))?,
        Some("json") => serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse JSON config: {}", e))?,
        Some("toml") => toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse TOML config: {}", e))?,
        Some("proxy") => {
            // Caddy-like DSL format
            use crate::config::dsl_converter::convert_dsl_to_config;
            use crate::config::dsl_parser::parse_dsl;

            let dsl_config = parse_dsl(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse DSL config: {}", e))?;

            convert_dsl_to_config(dsl_config)
                .map_err(|e| anyhow::anyhow!("Failed to convert DSL to config: {}", e))?
        }
        _ => {
            anyhow::bail!("Unsupported config file format. Use .yaml, .json, .toml, or .proxy");
        }
    };

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_parsing() {
        let yaml = r#"
server:
  bind:
    - "0.0.0.0:8080"
  workers: "auto"
  protocols:
    - http1
    - http2
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.server.bind.len(), 1);
        assert_eq!(config.server.workers, "auto");
    }
}
