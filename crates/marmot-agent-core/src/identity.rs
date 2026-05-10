use nostr::{Keys, ToBech32};
use std::path::Path;
use tracing::{info, warn};
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
        let mut contents = secret.as_secret_bytes().to_vec();
        
        tokio::fs::write(path, hex::encode(&contents)).await?;
        if let Err(e) = set_perms(path).await {
            warn!("could not set restrictive permissions on identity file: {e}");
        }
        contents.fill(0); // zeroise in-memory copy
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

#[cfg(unix)]
async fn set_perms(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    tokio::fs::set_permissions(path, perms).await
}

#[cfg(not(unix))]
async fn set_perms(_path: &Path) -> std::io::Result<()> {
    Ok(())
}
