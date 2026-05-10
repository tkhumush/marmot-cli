use nostr::{Keys, ToBech32};
use std::path::Path;
use tracing::info;
use crate::Result;

/// A Nostr identity used by the agent.
#[derive(Debug, Clone)]
pub struct Identity {
    pub keys: Keys,
    pub name: Option<String>,
}

impl Identity {
    /// Generate a brand-new random identity.
    pub fn generate() -> Self {
        let keys = Keys::generate();
        Self { keys, name: None }
    }

    /// Generate a named identity.
    pub fn generate_named(name: impl Into<String>) -> Self {
        let mut id = Self::generate();
        id.name = Some(name.into());
        id
    }

    /// Load from a 32-byte secret stored on disk (hex or raw bytes).
    pub fn from_secret_hex(hex_str: &str) -> Result<Self> {
        let keys = Keys::parse(hex_str)
            .map_err(|e| crate::Error::Identity(format!("invalid secret key: {e}")))?;
        Ok(Self { keys, name: None })
    }

    /// Save the raw secret key bytes to a file with 0o600 permissions.
    pub async fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let secret = self.keys.secret_key();
        let mut raw = secret.as_secret_bytes().to_vec();
        let mut encoded = hex::encode(&raw).into_bytes();
        raw.fill(0);
        write_secret_file(path, &encoded).await?;
        encoded.fill(0);
        info!("identity saved to {}", path.display());
        Ok(())
    }

    /// Load from a file.
    pub async fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let hex_str = tokio::fs::read_to_string(path).await?;
        let hex_str = hex_str.trim();
        Self::from_secret_hex(hex_str)
    }

    pub fn public_key_hex(&self) -> String {
        self.keys.public_key().to_hex()
    }

    pub fn nsec(&self) -> String {
        self.keys.secret_key().to_bech32().unwrap_or_default()
    }
    
    pub fn npub(&self) -> String {
        self.keys.public_key().to_bech32().unwrap_or_default()
    }
}

/// Write `data` to `path` creating it with 0o600 permissions atomically on Unix.
/// On non-Unix platforms falls back to a plain write (no chmod support).
#[cfg(unix)]
async fn write_secret_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
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
async fn write_secret_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    tokio::fs::write(path, data).await
}
