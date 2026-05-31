use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub default_vault: Option<PathBuf>,
    pub leader: Option<String>,
    pub format_on_save: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_vault: None,
            leader: None,
            format_on_save: true,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config {}", path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }
}

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("onl", "nubo", "tesoro")
}

fn config_path() -> Option<PathBuf> {
    project_dirs().map(|d| d.config_dir().join("config.json"))
}

pub fn cache_dir() -> Option<PathBuf> {
    project_dirs().map(|d| d.cache_dir().to_path_buf())
}

pub fn resolve_vault(cli_vault: Option<PathBuf>, cfg: &Config) -> Result<PathBuf> {
    let raw = cli_vault
        .or_else(|| std::env::var_os("TESORO_VAULT").map(PathBuf::from))
        .or_else(|| cfg.default_vault.clone())
        .unwrap_or_else(|| PathBuf::from("."));
    let abs = std::fs::canonicalize(&raw)
        .with_context(|| format!("resolving vault path {}", raw.display()))?;
    anyhow::ensure!(
        abs.is_dir(),
        "vault path is not a directory: {}",
        abs.display()
    );
    Ok(abs)
}
