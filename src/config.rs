use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub general: GeneralConfig,
    pub backend: BackendConfig,
    pub output: OutputConfig,
    pub scan: ScanConfig,
    pub kill: KillConfig,
    pub ui: UiConfig,
    pub history: HistoryConfig,
}
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct GeneralConfig {
    pub default_port: Option<u16>,
}
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct BackendConfig {
    pub strategy: BackendStrategy,
}
#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackendStrategy {
    #[default]
    Auto,
    Lsof,
    Netstat,
    Ss,
}
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct OutputConfig {
    pub color: Option<bool>,
    pub verbose: bool,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct ScanConfig {
    pub default_range: Option<String>,
}
impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            default_range: Some("3000-9999".into()),
        }
    }
}
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct KillConfig {
    pub default_signal: Option<String>,
    pub confirm: Option<bool>,
}
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub color: Option<bool>,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryConfig {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub max_entries: usize,
}
impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            max_entries: 200,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.general.default_port != Some(0),
            "Invalid config: default_port must be nonzero"
        );
        anyhow::ensure!(
            (1..=10_000).contains(&self.history.max_entries),
            "Invalid config: history.max_entries must be 1..=10000"
        );
        if let Some(s) = &self.kill.default_signal {
            anyhow::ensure!(
                ["INT", "TERM", "KILL"].contains(&s.to_uppercase().as_str()),
                "Invalid config: default_signal must be INT, TERM or KILL"
            );
        }
        if let Some(range) = &self.scan.default_range {
            let (from, to) = range
                .split_once('-')
                .context("Invalid config: scan.default_range must be FROM-TO")?;
            let from: u16 = from.parse().context("Invalid config: range start")?;
            let to: u16 = to.parse().context("Invalid config: range end")?;
            anyhow::ensure!(from > 0 && from <= to, "Invalid config: port range");
        }
        Ok(())
    }
}
pub fn load_config(cli_path: Option<&PathBuf>) -> Result<Config> {
    if let Some(path) = cli_path {
        return load_from_file(path);
    }
    let local = PathBuf::from("crossport.toml");
    if local.exists() {
        return load_from_file(&local);
    }
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".config/crossport/config.toml");
        if path.exists() {
            return load_from_file(&path);
        }
    }
    Ok(Config::default())
}
fn load_from_file(path: &std::path::Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config: Config = toml::from_str(&content).context("Invalid config file")?;
    config.validate()?;
    Ok(config)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_typos_and_invalid_policy() {
        assert!(toml::from_str::<Config>("[output]\ncolour = true").is_err());
        for value in [
            "[history]\nmax_entries = 0",
            "[scan]\ndefault_range = '8000-3000'",
            "[kill]\ndefault_signal = 'SIGTERM'",
            "[general]\ndefault_port = 0",
        ] {
            assert!(toml::from_str::<Config>(value).unwrap().validate().is_err());
        }
    }
    #[test]
    fn supports_all_settings() {
        let c: Config = toml::from_str("[general]\ndefault_port=8088\n[backend]\nstrategy='lsof'\n[output]\ncolor=false\nverbose=true\n[ui]\ncolor=false\n[history]\nenabled=true\nmax_entries=5").unwrap();
        c.validate().unwrap();
        assert_eq!(c.general.default_port, Some(8088));
        assert_eq!(c.backend.strategy, BackendStrategy::Lsof);
        assert!(!c.ui.color.unwrap());
        assert!(c.output.verbose);
    }
}
