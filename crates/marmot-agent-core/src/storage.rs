//! Agent storage layer: identity management + MDK storage backend.
//!
//! Uses platform-appropriate directories (XDG on Linux, etc.):
//!   - config:  ~/.config/marmot-cli/
//!   - data:    ~/.local/share/marmot-cli/
//!   - state:   ~/.local/state/marmot-cli/

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::identity::Identity;
use crate::Result;

/// Platform directories for marmot-cli.
pub struct AgentDirs {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
}

impl AgentDirs {
    /// Create / discover platform directories.
    pub fn new() -> Option<Self> {
        ProjectDirs::from("com", "tkhumush", "marmot-cli").map(|pd| Self {
            config_dir: pd.config_dir().to_path_buf(),
            data_dir: pd.data_dir().to_path_buf(),
            state_dir: pd.state_dir().unwrap_or(pd.data_dir()).to_path_buf(),
        })
    }

    /// Ensure all directories exist.
    pub async fn ensure(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.config_dir).await?;
        tokio::fs::create_dir_all(&self.data_dir).await?;
        tokio::fs::create_dir_all(&self.state_dir).await?;
        Ok(())
    }

    /// Path to the global config file.
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Path to the identities subdirectory.
    pub fn identities_dir(&self) -> PathBuf {
        self.data_dir.join("identities")
    }

    /// Path to an identity file by name.
    pub fn identity_file(&self, name: &str) -> PathBuf {
        self.identities_dir().join(format!("{}.json", name))
    }

    /// Path to the SQLite database.
    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("marmot.db")
    }

    /// Path to the database encryption key.
    pub fn db_key_path(&self) -> PathBuf {
        self.data_dir.join("db.key")
    }
}

/// Global agent configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentConfig {
    pub default_identity: Option<String>,
    pub default_relays: Vec<String>,
    pub daemon_listen: String,
}

impl AgentConfig {
    /// Load from config file or return defaults.
    pub async fn load(dirs: &AgentDirs) -> Self {
        let path = dirs.config_file();
        if path.exists() {
            match tokio::fs::read_to_string(&path).await {
                Ok(contents) => match toml::from_str(&contents) {
                    Ok(cfg) => return cfg,
                    Err(e) => warn!("invalid config file at {}: {}", path.display(), e),
                },
                Err(e) => warn!("could not read config file: {}", e),
            }
        }
        Self {
            default_relays: crate::Config::default_relays(),
            daemon_listen: "127.0.0.1:9222".to_string(),
            ..Default::default()
        }
    }

    /// Save to config file.
    pub async fn save(&self, dirs: &AgentDirs) -> Result<()> {
        let contents = toml::to_string_pretty(self)
            .map_err(|e| crate::Error::Serialization(format!("TOML serialization failed: {}", e)))?;
        tokio::fs::write(dirs.config_file(), contents).await?;
        Ok(())
    }
}

/// Serializable identity record stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRecord {
    pub name: String,
    pub npub: String,
    pub public_key_hex: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Storage manager for agent data.
pub struct AgentStorage {
    pub dirs: AgentDirs,
    pub config: AgentConfig,
}

impl AgentStorage {
    /// Initialize storage (create dirs, load config).
    pub async fn init() -> Result<Self> {
        let dirs = AgentDirs::new().ok_or_else(|| {
            crate::Error::Identity("could not determine platform directories".to_string())
        })?;
        dirs.ensure().await?;
        let config = AgentConfig::load(&dirs).await;
        info!("agent storage initialized at {}", dirs.data_dir.display());
        Ok(Self { dirs, config })
    }

    /// Save an identity to disk (secret key + metadata).
    pub async fn save_identity(&self, identity: &Identity) -> Result<()> {
        let dir = self.dirs.identities_dir();
        tokio::fs::create_dir_all(&dir).await?;

        let name = identity.name.as_deref().unwrap_or("default");
        let path = self.dirs.identity_file(name);

        // Save secret key in a separate .nsec file with restricted perms
        let secret_path = dir.join(format!("{}.nsec", name));
        identity.save_to_file(&secret_path).await?;

        // Save metadata JSON
        let record = IdentityRecord {
            name: name.to_string(),
            npub: identity.npub(),
            public_key_hex: identity.public_key_hex(),
            created_at: chrono::Utc::now().to_rfc3339(),
            metadata: None,
        };
        let json = serde_json::to_string_pretty(&record)
            .map_err(|e| crate::Error::Serialization(format!("JSON serialization failed: {}", e)))?;
        tokio::fs::write(&path, json).await?;

        info!("identity '{}' saved", name);
        Ok(())
    }

    /// Load an identity by name.
    pub async fn load_identity(&self, name: &str) -> Result<Identity> {
        let secret_path = self.dirs.identities_dir().join(format!("{}.nsec", name));
        if !secret_path.exists() {
            return Err(crate::Error::Identity(format!(
                "identity '{}' not found at {}",
                name,
                secret_path.display()
            )));
        }
        let mut id = Identity::load_from_file(&secret_path).await?;

        // Load metadata if available
        let meta_path = self.dirs.identity_file(name);
        if let Ok(json) = tokio::fs::read_to_string(&meta_path).await {
            if let Ok(record) = serde_json::from_str::<IdentityRecord>(&json) {
                id.name = Some(record.name);
            }
        }
        Ok(id)
    }

    /// List all saved identity names.
    pub async fn list_identities(&self) -> Result<Vec<IdentityRecord>> {
        let dir = self.dirs.identities_dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut entries = tokio::fs::read_dir(&dir).await?;
        let mut records = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(json) = tokio::fs::read_to_string(&path).await {
                    if let Ok(record) = serde_json::from_str::<IdentityRecord>(&json) {
                        records.push(record);
                    }
                }
            }
        }
        records.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(records)
    }

    /// Delete an identity.
    pub async fn delete_identity(&self, name: &str) -> Result<()> {
        let dir = self.dirs.identities_dir();
        let paths = [
            dir.join(format!("{}.nsec", name)),
            dir.join(format!("{}.json", name)),
        ];
        for p in &paths {
            if p.exists() {
                tokio::fs::remove_file(p).await?;
            }
        }
        info!("identity '{}' deleted", name);
        Ok(())
    }

    /// Get the default identity if set.
    pub async fn default_identity(&self) -> Result<Option<Identity>> {
        if let Some(ref name) = self.config.default_identity {
            match self.load_identity(name).await {
                Ok(id) => return Ok(Some(id)),
                Err(e) => warn!("default identity '{}' failed to load: {}", name, e),
            }
        }
        Ok(None)
    }

    /// Set the default identity name.
    pub async fn set_default_identity(&mut self, name: &str) -> Result<()> {
        // Verify it exists
        self.load_identity(name).await?;
        self.config.default_identity = Some(name.to_string());
        self.config.save(&self.dirs).await?;
        info!("default identity set to '{}'", name);
        Ok(())
    }

    /// Returns the 32-byte database encryption key, generating and persisting one if absent.
    /// Key is stored at ~/.local/share/marmot-cli/db.key with 0o600 permissions.
    pub async fn db_encryption_key(&self) -> Result<[u8; 32]> {
        let key_path = self.dirs.db_key_path();
        if key_path.exists() {
            let hex_str = tokio::fs::read_to_string(&key_path).await?;
            let hex_str = hex_str.trim();
            let bytes = hex::decode(hex_str).map_err(|e| {
                crate::Error::Serialization(format!("invalid db key hex: {}", e))
            })?;
            if bytes.len() != 32 {
                return Err(crate::Error::Serialization(
                    "db key must be 32 bytes".to_string(),
                ));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }

        // Generate new key
        let mut key = [0u8; 32];
        getrandom::getrandom(&mut key).map_err(|e| {
            crate::Error::Storage(format!("failed to generate db key: {}", e).into())
        })?;

        let mut hex_key = hex::encode(key).into_bytes();
        write_secret_file(&key_path, &hex_key).await?;
        hex_key.fill(0);

        Ok(key)
    }
}

/// Write `data` to `path` creating it with 0o600 permissions atomically on Unix.
#[cfg(unix)]
async fn write_secret_file(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .await?;
    file.write_all(data).await
}

#[cfg(not(unix))]
async fn write_secret_file(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    tokio::fs::write(path, data).await
}
