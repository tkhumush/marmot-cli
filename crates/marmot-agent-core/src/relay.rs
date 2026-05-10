use tracing::info;

/// Default relay list inherited from the White Noise messenger.
pub const DEFAULT_RELAYS: [&str; 3] = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.damus.io",
];

/// Relay manager placeholder: full Nostr relay integration will be wired
/// through MDK (which already handles relay pools correctly).
#[derive(Debug, Clone, Default)]
pub struct RelayManager;

impl RelayManager {
    pub fn new() -> Self {
        Self
    }
    pub async fn add(&self, url: &str) {
        info!("relay add requested: {}", url);
    }
    pub async fn add_defaults(&self) {
        for url in DEFAULT_RELAYS {
            self.add(url).await;
        }
    }
    pub async fn publish(&self, _event: nostr::Event) {
        info!("relay publish requested");
    }
}
