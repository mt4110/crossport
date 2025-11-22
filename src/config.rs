use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub kill: KillConfig,
    #[serde(default)]
    #[allow(dead_code)]
    pub ui: UiConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScanConfig {
    pub default_range: Option<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            default_range: Some("3000-9999".to_string()),
        }
    }
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct KillConfig {
    pub default_signal: Option<String>,
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[allow(dead_code)]
pub struct UiConfig {
    pub color: Option<bool>,
}

pub fn load_config(cli_path: Option<&PathBuf>) -> Result<Config> {
    // 1. CLI
    if let Some(path) = cli_path {
        if path.exists() {
            return load_from_file(path);
        }
    }

    // 2. Local (crossport.toml)
    let local_path = PathBuf::from("crossport.toml");
    if local_path.exists() {
        return load_from_file(&local_path);
    }

    // 3. Home
    if let Some(home) = dirs::home_dir() {
        let home_path = home.join(".config").join("crossport").join("config.toml");
        if home_path.exists() {
            return load_from_file(&home_path);
        }
    }

    // 4. Default
    Ok(Config::default())
}

fn load_from_file(path: &std::path::Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;
    let config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {:?}", path))?;
    Ok(config)
}
