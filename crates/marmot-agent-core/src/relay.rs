use nostr::{Alphabet, Event, Filter, Keys, Kind, PublicKey, RelayUrl, SingleLetterTag};
use nostr_relay_pool::{relay::ReqExitPolicy, RelayPool, RelayOptions};
use nostr_relay_pool::pool::RelayPoolBuilder;
use std::sync::Arc;
use tracing::{info, warn};

/// Fetch raw events matching an arbitrary filter. For diagnostics only.
pub async fn fetch_raw(filter: Filter, relays: &[&str]) -> crate::Result<Vec<Event>> {
    let pool = RelayPool::default();
    for url in relays {
        match RelayUrl::parse(url) {
            Ok(relay_url) => {
                if let Err(e) = pool.add_relay(relay_url, RelayOptions::default()).await {
                    warn!("failed to add relay {}: {}", url, e);
                }
            }
            Err(e) => warn!("invalid relay URL {}: {}", url, e),
        }
    }
    pool.connect().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    let events = match pool.fetch_events(
        vec![filter],
        std::time::Duration::from_secs(8),
        ReqExitPolicy::ExitOnEOSE,
    ).await {
        Ok(evs) => evs.to_vec(),
        Err(e) => {
            warn!("fetch_raw failed: {}", e);
            vec![]
        }
    };
    pool.disconnect().await;
    Ok(events)
}

use crate::Result;

/// Default relay list inherited from the White Noise messenger.
pub const DEFAULT_RELAYS: [&str; 3] = [
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://relay.damus.io",
];

/// Publish a gift-wrap (kind 1059) to a recipient's inbox relays with NIP-42 auth.
/// Falls back to `default_relays` if inbox relays are unreachable.
pub async fn publish_gift_wrap(
    event: &Event,
    inbox_relays: &[String],
    fallback_relays: &[&str],
    signer: &Keys,
) -> Result<Vec<(String, bool)>> {
    // Try inbox relays first with NIP-42 auth.
    if !inbox_relays.is_empty() {
        let mut builder = RelayPoolBuilder::default();
        builder.__signer = Some(Arc::new(signer.clone()));
        let pool = builder.build();
        let mut any_added = false;
        for url in inbox_relays {
            if let Ok(relay_url) = RelayUrl::parse(url) {
                if pool.add_relay(relay_url, RelayOptions::default()).await.is_ok() {
                    any_added = true;
                }
            }
        }
        if any_added {
            pool.connect().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            match pool.send_event(event).await {
                Ok(_) => {
                    info!("gift-wrap published to inbox relays: {}", event.id);
                    let results = inbox_relays.iter().map(|u| (u.clone(), true)).collect();
                    pool.disconnect().await;
                    return Ok(results);
                }
                Err(e) => {
                    warn!("gift-wrap inbox publish failed ({}), falling back", e);
                }
            }
            pool.disconnect().await;
        }
    }

    // Fallback to default relays without auth.
    publish_event(event, fallback_relays).await
}

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

/// Fetch group-related events (kind 445 / 10449 / 4459) from relays matching given h-tags.
/// Returns events newest first, limited by `limit`.
pub async fn fetch_group_events(
    h_tags: &[String],
    limit: usize,
    relays: &[&str],
) -> Result<Vec<Event>> {
    let pool = RelayPool::default();
    for url in relays {
        match RelayUrl::parse(url) {
            Ok(relay_url) => {
                if let Err(e) = pool.add_relay(relay_url, RelayOptions::default()).await {
                    warn!("failed to add relay {}: {}", url, e);
                }
            }
            Err(e) => warn!("invalid relay URL {}: {}", url, e),
        }
    }
    pool.connect().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let filter = Filter::new()
        .kinds(vec![Kind::Custom(445), Kind::Custom(10449), Kind::Custom(4459)])
        .limit(limit);
    let filter = if !h_tags.is_empty() {
        filter.custom_tags(SingleLetterTag::lowercase(Alphabet::H), h_tags.iter().cloned())
    } else {
        filter
    };

    let mut all_events = Vec::new();
    match pool.fetch_events(
        vec![filter],
        std::time::Duration::from_secs(8),
        ReqExitPolicy::ExitOnEOSE,
    ).await {
        Ok(events) => all_events.extend(events.into_iter()),
        Err(e) => warn!("Failed to fetch group events: {}", e),
    }
    pool.disconnect().await;

    // Deduplicate by event id
    let mut seen = std::collections::HashSet::new();
    all_events.retain(|ev| seen.insert(ev.id));
    // Sort newest first
    all_events.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(all_events)
}

/// Fetch a user's NIP-17 inbox relay list (kind 10050).
/// Returns the relay URLs from `relay` tags, or an empty vec if none found.
pub async fn fetch_inbox_relays(pubkey: PublicKey, search_relays: &[&str]) -> Vec<String> {
    let pool = RelayPool::default();
    for url in search_relays {
        if let Ok(relay_url) = RelayUrl::parse(url) {
            let _ = pool.add_relay(relay_url, RelayOptions::default()).await;
        }
    }
    pool.connect().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let filter = Filter::new().kind(Kind::Custom(10050)).author(pubkey).limit(1);
    let events = pool
        .fetch_events(vec![filter], std::time::Duration::from_secs(5), ReqExitPolicy::ExitOnEOSE)
        .await
        .unwrap_or_default();
    pool.disconnect().await;

    events
        .into_iter()
        .next()
        .map(|e| {
            e.tags
                .iter()
                .filter(|t| t.kind() == nostr::TagKind::Relay)
                .filter_map(|t| t.content().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch gift-wrap events (kind 1059) addressed to `pubkey` from relays.
/// These carry NIP-59 sealed welcome messages (kind 444) for MLS group joining.
pub async fn fetch_gift_wrap_events(
    pubkey: PublicKey,
    relays: &[&str],
) -> Result<Vec<Event>> {
    let pool = RelayPool::default();
    for url in relays {
        match RelayUrl::parse(url) {
            Ok(relay_url) => {
                if let Err(e) = pool.add_relay(relay_url, RelayOptions::default()).await {
                    warn!("failed to add relay {}: {}", url, e);
                }
            }
            Err(e) => warn!("invalid relay URL {}: {}", url, e),
        }
    }
    pool.connect().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let filter = Filter::new()
        .kind(Kind::GiftWrap)
        .pubkey(pubkey)
        .limit(200);

    let events = match pool.fetch_events(
        vec![filter],
        std::time::Duration::from_secs(8),
        ReqExitPolicy::ExitOnEOSE,
    ).await {
        Ok(evs) => evs.to_vec(),
        Err(e) => {
            warn!("Failed to fetch gift wrap events: {}", e);
            return Ok(vec![]);
        }
    };
    pool.disconnect().await;
    Ok(events)
}
