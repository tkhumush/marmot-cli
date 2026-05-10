use nostr::{Event, RelayUrl};
use nostr_relay_pool::{RelayPool, RelayOptions};
use tracing::{info, warn};

use crate::Result;

/// Default relay list inherited from the White Noise messenger.
pub const DEFAULT_RELAYS: [&str; 3] = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.damus.io",
];

/// Publish a Nostr event to a set of relays via RelayPool.
pub async fn publish_event(
    event: &Event,
    relays: &[&str],
) -> Result<Vec<(String, bool)>> {
    let pool = RelayPool::default();
    let mut results = Vec::new();

    for url in relays {
        match RelayUrl::parse(url) {
            Ok(relay_url) => {
                if let Err(e) = pool.add_relay(relay_url, RelayOptions::default()).await {
                    warn!("failed to add relay {}: {}", url, e);
                    results.push((url.to_string(), false));
                    continue;
                }
            }
            Err(e) => {
                warn!("invalid relay URL {}: {}", url, e);
                results.push((url.to_string(), false));
                continue;
            }
        }
    }

    pool.connect().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    match pool.send_event(event).await {
        Ok(_) => {
            info!("event published: {}", event.id);
            for url in relays {
                results.push((url.to_string(), true));
            }
        }
        Err(e) => {
            warn!("failed to publish event: {}", e);
            for url in relays {
                results.push((url.to_string(), false));
            }
        }
    }

    pool.disconnect().await;
    Ok(results)
}
