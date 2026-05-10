use nostr_relay_pool::prelude::*;
use nostr::{Event, Filter, RelayUrl};
use tracing::{info, warn, debug};
use crate::Result;

/// Default relay list inherited from the White Noise messenger.
pub const DEFAULT_RELAYS: [&str; 3] = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.damus.io",
];

/// Light-weight relay manager: connects, subscribes, publishes.
#[derive(Debug)]
pub struct RelayManager {
    pool: RelayPool,
}

impl RelayManager {
    pub fn new() -> Self {
        let pool = RelayPool::new(RelayPoolOptions::default());
        Self { pool }
    }

    /// Add and connect to a relay.
    pub async fn add(&self, url: &str) -> Result<()> {
        let url = RelayUrl::parse(url)
            .map_err(|e| crate::Error::Relay(format!("invalid relay url: {e}")))?;
        let opts = RelayOptions::new();
        self.pool.add_relay(url, opts).await
            .map_err(|e| crate::Error::Relay(format!("failed to add relay: {e}")))?;
        info!("relay connected: {}", url);
        Ok(())
    }

    /// Add all default relays.
    pub async fn add_defaults(&self) -> Result<()> {
        for url in DEFAULT_RELAYS {
            if let Err(e) = self.add(url).await {
                warn!("failed to add default relay {}: {}", url, e);
            }
        }
        Ok(())
    }

    /// Publish a Nostr event to all connected relays.
    pub async fn publish(&self, event: Event) -> Result<()> {
        let ids = self.pool.send_event(event).await
            .map_err(|e| crate::Error::Relay(format!("publish failed: {e}")))?;
        debug!("event published to {} relays", ids.len());
        Ok(())
    }

    /// Subscribe to events matching a filter.
    pub async fn subscribe(&self, filter: Filter) -> Result<()> {
        let id = SubscriptionId::new(uuid::Uuid::new_v4().to_string());
        self.pool.subscribe(id, vec![filter], None).await
            .map_err(|e| crate::Error::Relay(format!("subscribe failed: {e}")))?;
        Ok(())
    }

    /// Convenience: subscribe to KeyPackage events for a pubkey.
    pub async fn subscribe_keypackages(&self, pubkey: &nostr::PublicKey) -> Result<()> {
        let filter = Filter::new()
            .kind(nostr::Kind::Custom(30443))
            .author(pubkey);
        self.subscribe(filter).await
    }

    /// Receive notifications (events, relay status, etc.).
    pub fn notifications(&self) -> nostr_relay_pool::prelude::RelayPoolNotification {
        self.pool.notifications()
    }
}