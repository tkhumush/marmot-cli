use nostr::{Event, Filter, Kind, PublicKey, RelayUrl};
use nostr_relay_pool::{relay::ReqExitPolicy, RelayPool, RelayOptions};
use tracing::{info, warn};

use crate::Result;

/// Default relay list inherited from the White Noise messenger.
pub const DEFAULT_RELAYS: [&str; 3] = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.damus.io",
];

/// Publish a single Nostr event to a set of relays via RelayPool.
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

/// Publish multiple Nostr events in sequence to a set of relays.
/// Returns per-event results: (event_label, Vec<(relay_url, ok)>).
///
/// # Arguments
/// * `events` - List of (label, event) tuples for traceability
/// * `relays` - Relay URLs as string slices
pub async fn publish_events<'a>(
    events: &'a [(&'a str, &'a Event)],
    relays: &'a [&'a str],
) -> Result<Vec<(&'a str, Vec<(String, bool)>)>> {
    let pool = RelayPool::default();

    // Add all relays once
    for url in relays {
        match RelayUrl::parse(url) {
            Ok(relay_url) => {
                if let Err(e) = pool.add_relay(relay_url, RelayOptions::default()).await {
                    warn!("failed to add relay {}: {}", url, e);
                }
            }
            Err(e) => {
                warn!("invalid relay URL {}: {}", url, e);
            }
        }
    }

    pool.connect().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let mut all_results = Vec::new();

    for (label, event) in events {
        // The label here is used for traceability in logs.
        let _label = label; // prefix: label string
        match pool.send_event(event).await {
            Ok(_) => {
                info!("{} published: {}", label, event.id);
                let sub_results = relays.iter().map(|url| (url.to_string(), true)).collect();
                all_results.push((*label, sub_results));
            }
            Err(e) => {
                warn!("failed to publish {}: {}", label, e);
                let sub_results = relays.iter().map(|url| (url.to_string(), false)).collect();
                all_results.push((*label, sub_results));
            }
        }
    }

    pool.disconnect().await;
    Ok(all_results)
}

/// Fetch the latest KeyPackage event (kind 30443) for a given user from relays.
pub async fn fetch_keypackage(
    pubkey: PublicKey,
    relays: &[&str],
) -> Result<Option<Event>> {
    let pool = RelayPool::default();

    for url in relays {
        match RelayUrl::parse(url) {
            Ok(relay_url) => {
                if let Err(e) = pool.add_relay(relay_url, RelayOptions::default()).await {
                    warn!("failed to add relay {}: {}", url, e);
                }
            }
            Err(e) => {
                warn!("invalid relay URL {}: {}", url, e);
            }
        }
    }

    pool.connect().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let filter = Filter::new()
        .kind(Kind::Custom(30443))
        .author(pubkey);

    let events = match pool.fetch_events(
        vec![filter],
        std::time::Duration::from_secs(5),
        ReqExitPolicy::ExitOnEOSE,
    ).await {
        Ok(events) => events.to_vec(),
        Err(e) => {
            warn!("Failed to query keypackage: {}", e);
            return Ok(None);
        }
    };

    pool.disconnect().await;
    Ok(events.into_iter().next())
}
