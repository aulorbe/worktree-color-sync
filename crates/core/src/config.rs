use crate::paths::expand_tilde;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub daemon: DaemonConfig,
    pub integrations: IntegrationsConfig,
    pub colors: ColorsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub socket_path: String,
    pub state_path: String,
    pub git_timeout_ms: u64,
    pub integration_timeout_ms: u64,
    pub neutral_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationsConfig {
    pub ghostty: GhosttyConfig,
    pub cursor: CursorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhosttyConfig {
    pub enabled: bool,
    pub overrides_dir: String,
    pub global_fallback_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorsConfig {
    pub palette: Option<Vec<String>>,
    pub strict_palette: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig {
                socket_path: "~/.local/run/worktree-sync.sock".to_string(),
                state_path: "~/.local/state/worktree-sync/state.json".to_string(),
                git_timeout_ms: 500,
                integration_timeout_ms: 500,
                neutral_color: "#1f1f1f".to_string(),
            },
            integrations: IntegrationsConfig {
                ghostty: GhosttyConfig {
                    enabled: true,
                    overrides_dir: "~/.config/ghostty/worktree-sync.d".to_string(),
                    global_fallback_path: "~/.config/ghostty/worktree-sync-global.conf".to_string(),
                },
                cursor: CursorConfig { enabled: true },
            },
            colors: ColorsConfig {
                palette: None,
                strict_palette: false,
            },
        }
    }
}

impl Config {
    pub fn load(path: Option<&str>) -> Result<Self> {
        let Some(path) = path else {
            return Ok(Self::default());
        };

        let config_path = expand_tilde(path)?;
        if !config_path.exists() {
            return Ok(Self::default());
        }

        let raw = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config {}", config_path.display()))?;
        let mut parsed: Self = toml::from_str(&raw)
            .with_context(|| format!("failed to parse config {}", config_path.display()))?;

        parsed.normalize()?;
        Ok(parsed)
    }

    fn normalize(&mut self) -> Result<()> {
        self.daemon.neutral_color = normalize_hex(&self.daemon.neutral_color)?;

        if let Some(palette) = &mut self.colors.palette {
            for color in palette.iter_mut() {
                *color = normalize_hex(color)?;
            }
        }

        Ok(())
    }

    pub fn socket_path(&self) -> Result<PathBuf> {
        expand_tilde(&self.daemon.socket_path)
    }

    pub fn state_path(&self) -> Result<PathBuf> {
        expand_tilde(&self.daemon.state_path)
    }

    pub fn ghostty_overrides_dir(&self) -> Result<PathBuf> {
        expand_tilde(&self.integrations.ghostty.overrides_dir)
    }

    pub fn ghostty_global_fallback_path(&self) -> Result<PathBuf> {
        expand_tilde(&self.integrations.ghostty.global_fallback_path)
    }

    pub fn palette(&self) -> Vec<String> {
        self.colors
            .palette
            .clone()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }
}

fn normalize_hex(input: &str) -> Result<String> {
    let trimmed = input.trim();
    let with_hash = if trimmed.starts_with('#') {
        trimmed.to_string()
    } else {
        format!("#{trimmed}")
    };

    if with_hash.len() != 7 || !with_hash.chars().skip(1).all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("invalid hex color: {input}");
    }

    Ok(with_hash.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_values() {
        let cfg = Config::default();
        assert!(cfg.daemon.socket_path.contains("worktree-sync.sock"));
        assert_eq!(cfg.daemon.neutral_color, "#1f1f1f");
    }
}
