
pub mod identity;
pub mod relay;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub default_relays: Vec<String>,
    pub identity_path: String,
}

impl Config {
    /// Returns the default relay list inherited from the White Noise app.
    pub fn default_relays() -> Vec<String> {
        vec![
            "wss://nos.lol".to_string(),
            "wss://relay.primal.net".to_string(),
            "wss://relay.damus.io".to_string(),
        ]
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("relay error: {0}")]
    Relay(String),
    #[error("identity error: {0}")]
    Identity(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("anyhow: {0}")]
    Any(#[from] anyhow::Error),
}

/// Convenience re-export of the core prelude.
pub mod prelude {
    pub use crate::{Config, Error, Result};
}
