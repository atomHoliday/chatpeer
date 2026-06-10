use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub identity: IdentityConfig,
    pub network: NetworkConfig,
    pub db: DbConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityConfig {
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub listen_on: Vec<String>,
    pub relay_peers: Vec<String>,
    pub bootstrap_peers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chatpeer");
        let _config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chatpeer");

        Self {
            identity: IdentityConfig {
                username: std::env::var("USER")
                    .unwrap_or_else(|_| "user".to_string()),
            },
            network: NetworkConfig {
                listen_on: vec![
                    "/ip4/0.0.0.0/tcp/0".into(),
                    "/ip6/::/tcp/0".into(),
                ],
                relay_peers: vec![],
                bootstrap_peers: vec![],
            },
            db: DbConfig {
                path: data_dir.join("messages.db"),
            },
        }
    }
}

impl Config {
    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chatpeer")
            .join("config.toml")
    }

    pub fn load_or_default() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("reading config from {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("parsing config from {}", path.display()))?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            tracing::info!("created default config at {}", path.display());
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating config dir {}", parent.display()))?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("writing config to {}", path.display()))?;
        Ok(())
    }

    pub fn data_dir(&self) -> PathBuf {
        self.db
            .path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
