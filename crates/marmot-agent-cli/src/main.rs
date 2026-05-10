use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::Level;
use nostr::{Event, PublicKey};
use mdk_core::GroupId;

#[derive(Parser)]
#[command(name = "marmot-cli")]
#[command(about = "A headless Marmot messaging agent, inspired by signal-cli.")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, help = "Increase verbosity")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new Nostr identity or list existing ones.
    Identity {
        #[command(subcommand)]
        action: IdentityAction,
    },
    /// Manage relay connections.
    Relay {
        #[command(subcommand)]
        action: RelayAction,
    },
    /// Publish your KeyPackage to relays.
    Keypackage {
        #[command(subcommand)]
        action: KeypackageAction,
    },
    /// Start the background daemon.
    Daemon {
        #[arg(short, long, default_value = "127.0.0.1:9222")]
        listen: String,
    },
    /// Group management.
    Groups {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Direct message (DM) — creates a 2-member group.
    Dm {
        #[command(subcommand)]
        action: DmAction,
    },
}

#[derive(Subcommand)]
enum IdentityAction {
    /// Create a new identity.
    Create {
        #[arg(short, long, help = "Human-readable name")]
        name: Option<String>,
    },
    /// List saved identities.
    List,
    /// Show details of a saved identity.
    Show {
        #[arg(help = "Identity name")]
        name: String,
    },
    /// Delete an identity.
    Delete {
        #[arg(help = "Identity name")]
        name: String,
    },
    /// Set the default identity.
    SetDefault {
        #[arg(help = "Identity name")]
        name: String,
    },
}

#[derive(Subcommand)]
enum RelayAction {
    List,
    Add { url: String },
}

#[derive(Subcommand)]
enum KeypackageAction {
    Publish,
    Show,
}

#[derive(Subcommand)]
enum GroupAction {
    List,
    Create {
        #[arg(short, long, help = "Group name")]
        name: String,
        #[arg(long, help = "Also publish the group creation events to relays")]
        publish: bool,
    },
    Invite {
        #[arg(short, long, help = "Group ID")]
        group: String,
        #[arg(short, long, help = "Member npub")]
        member: String,
    },
}

#[derive(Subcommand)]
enum DmAction {
    /// Start a DM with someone by npub.
    Create {
        #[arg(short, long, help = "Recipient npub")]
        recipient: String,
        #[arg(long, help = "Also publish the DM group creation events to relays")]
        publish: bool,
    },
    /// List all conversation groups.
    List,
    /// Send a message to a DM group.
    Send {
        #[arg(short, long, help = "Group ID (hex or npub of recipient)")]
        group: String,
        #[arg(short, long, help = "Message content")]
        message: String,
        #[arg(long, help = "Also publish the message to group relays")]
        publish: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .init();

    match cli.command {
        Commands::Identity { action } => match action {
            IdentityAction::Create { name } => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                let id = if let Some(n) = name {
                    marmot_agent_core::identity::Identity::generate_named(n)
                } else {
                    marmot_agent_core::identity::Identity::generate()
                };

                if let Err(e) = storage.save_identity(&id).await {
                    eprintln!("Failed to save identity: {}", e);
                    return;
                }

                println!("Identity created");
                println!("  name: {}", id.name.as_deref().unwrap_or("default"));
                println!("  npub: {}", id.npub());
                println!("  nsec: {}", id.nsec());
                println!("\n  Saved to: {}", storage.dirs.identities_dir().display());
            }
            IdentityAction::List => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                match storage.list_identities().await {
                    Ok(records) => {
                        if records.is_empty() {
                            println!("No identities found.");
                            println!("  Create one with: marmot-cli identity create --name <name>");
                            return;
                        }
                        println!("Identities:");
                        for r in records {
                            let default_marker = if storage.config.default_identity.as_ref() == Some(&r.name) {
                                " (default)"
                            } else {
                                ""
                            };
                            println!("  {}{}", r.name, default_marker);
                            println!("    npub: {}", r.npub);
                            println!("    created: {}", r.created_at);
                        }
                    }
                    Err(e) => eprintln!("Failed to list identities: {}", e),
                }
            }
            IdentityAction::Show { name } => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                match storage.load_identity(&name).await {
                    Ok(id) => {
                        println!("Identity: {}", name);
                        println!("  npub: {}", id.npub());
                        println!("  nsec: {}", id.nsec());
                        println!("  pubkey (hex): {}", id.public_key_hex());
                    }
                    Err(e) => eprintln!("Failed to load identity '{}': {}", name, e),
                }
            }
            IdentityAction::Delete { name } => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                if let Err(e) = storage.delete_identity(&name).await {
                    eprintln!("Failed to delete identity '{}': {}", name, e);
                } else {
                    println!("Identity '{}' deleted.", name);
                }
            }
            IdentityAction::SetDefault { name } => {
                let mut storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                if let Err(e) = storage.set_default_identity(&name).await {
                    eprintln!("Failed to set default identity: {}", e);
                } else {
                    println!("Default identity set to '{}'.", name);
                }
            }
        },
        Commands::Relay { action } => match action {
            RelayAction::List => {
                println!("Default relays:");
                for url in marmot_agent_core::relay::DEFAULT_RELAYS {
                    println!("  {}", url);
                }
            }
            RelayAction::Add { url } => {
                println!("Adding relay {}... (not yet implemented)", url);
            }
        },
        Commands::Keypackage { action } => match action {
            KeypackageAction::Publish => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                let ctx = match marmot_agent_core::context::AgentContext::with_default(storage).await {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        eprintln!("No default identity set.");
                        eprintln!("  Create one:  marmot-cli identity create --name <name>");
                        eprintln!("  Set default: marmot-cli identity set-default <name>");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to load agent context: {}", e);
                        return;
                    }
                };

                // Parse default relays
                let relays: Vec<nostr::RelayUrl> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter()
                    .filter_map(|url| nostr::RelayUrl::parse(url).ok())
                    .collect();

                // Create KeyPackage via MDK
                let kp_data = match ctx.create_keypackage(relays.clone()) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("KeyPackage creation failed: {}", e);
                        return;
                    }
                };

                // Sign the event
                let event = match ctx.sign_keypackage_event(&kp_data) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Event signing failed: {}", e);
                        return;
                    }
                };

                // Publish to relays
                let results = match marmot_agent_core::relay::publish_event(
                    &event,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Publish failed: {}", e);
                        return;
                    }
                };

                println!("KeyPackage published!");
                println!("  Event ID: {}", event.id);
                println!("  d-tag: {}", kp_data.d_tag);
                println!("  Relay results:");
                for (url, ok) in results {
                    let status = if ok { "OK" } else { "FAIL" };
                    println!("    {} {}", status, url);
                }
            }
            KeypackageAction::Show => {
                println!("KeyPackage info... (not yet implemented)");
            }
        },
        Commands::Daemon { listen } => {
            println!("Starting daemon on {}...", listen);

            let handler: marmot_agent_rpc::server::HandlerFn = Arc::new(
                move |method: String, params: serde_json::Value| {
                    match method.as_str() {
                        "ping" => Ok(serde_json::json!({"pong": true})),
                        "identity_npub" => {
                            // TODO: load default identity from storage
                            Ok(serde_json::json!({"npub": null, "note": "not yet implemented"}))
                        }
                        "list_groups" => {
                            Ok(serde_json::json!({"groups": [], "note": "not yet implemented"}))
                        }
                        "send_message" => {
                            let _group_id = params.get("group_id").and_then(|v| v.as_str());
                            let _content = params.get("content").and_then(|v| v.as_str());
                            Ok(serde_json::json!({"sent": false, "note": "not yet implemented"}))
                        }
                        _ => Err(format!("Unknown method: {}", method)),
                    }
                }
            );

            println!("JSON-RPC methods available: ping, identity_npub, list_groups, send_message");
            println!("Press Ctrl+C to stop.");

            if let Err(e) = marmot_agent_rpc::server::serve_tcp_blocking(&listen,
                handler,
            ) {
                eprintln!("Daemon error: {}", e);
            }
        }
        Commands::Groups { action } => match action {
            GroupAction::List => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                let ctx = match marmot_agent_core::context::AgentContext::with_default(storage).await {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        eprintln!("No default identity set.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to load agent context: {}", e);
                        return;
                    }
                };

                match ctx.list_groups() {
                    Ok(groups) => {
                        if groups.is_empty() {
                            println!("No groups found.");
                        } else {
                            println!("Groups:");
                            for g in groups {
                                let name = if g.name.is_empty() { "unnamed" } else { &g.name };
                                println!("  Group '{}' (id: {:?})", name, g.mls_group_id);
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to list groups: {}", e),
                }
            }
            GroupAction::Create { name, publish } => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                let ctx = match marmot_agent_core::context::AgentContext::with_default(storage).await {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        eprintln!("No default identity set.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to load agent context: {}", e);
                        return;
                    }
                };

                let relays: Vec<nostr::RelayUrl> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter()
                    .filter_map(|url| nostr::RelayUrl::parse(url).ok())
                    .collect();

                match ctx.create_group(&name, relays) {
                    Ok(result) => {
                        println!("Group '{}' created!", name);
                        println!("  MLS group ID: {:?}", result.group.mls_group_id);

                        if publish {
                            let mut events_to_publish: Vec<(&str, Event)> = Vec::new();

                            // Sign any welcome rumors
                            for (i, rumor) in result.welcome_rumors.iter().enumerate() {
                                let event = match rumor
                                    .clone()
                                    .sign_with_keys(&ctx.identity.keys)
                                {
                                    Ok(e) => e,
                                    Err(e) => {
                                        eprintln!("Failed to sign welcome rumor {}: {}", i, e);
                                        continue;
                                    }
                                };
                                events_to_publish.push(("welcome", event));
                            }

                            if !events_to_publish.is_empty() {
                                println!("  Publishing welcome events to relays...");
                                let refs: Vec<(&str, &Event)> = events_to_publish
                                    .iter()
                                    .map(|(label, ev)| (*label, ev))
                                    .collect();

                                let results = match marmot_agent_core::relay::publish_events(
                                    &refs,
                                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                                ).await {
                                    Ok(r) => r,
                                    Err(e) => {
                                        eprintln!("Publish failed: {}", e);
                                        return;
                                    }
                                };

                                println!("  Publish results:");
                                for (label, relay_results) in results {
                                    let ok_count = relay_results.iter().filter(|(_, ok)| *ok).count();
                                    println!("    {}: {}/{} relays OK", label, ok_count, relay_results.len());
                                    for (url, ok) in relay_results {
                                        let status = if ok { "OK" } else { "FAIL" };
                                        println!("      {} {}", status, url);
                                    }
                                }
                            } else {
                                println!("  No welcome events to publish (group has no initial members).");
                            }
                        } else {
                            println!("  NOTE: Use --publish to send welcome events to relays.");
                        }
                    }
                    Err(e) => eprintln!("Group creation failed: {}", e),
                }
            }
            GroupAction::Invite { group, member } => {
                println!("Inviting {} to group {}... (not yet implemented)", member, group);
            }
        },
        Commands::Dm { action } => match action {
            DmAction::Create { recipient, publish } => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                let ctx = match marmot_agent_core::context::AgentContext::with_default(storage).await {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        eprintln!("No default identity set.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to load agent context: {}", e);
                        return;
                    }
                };

                // Parse recipient npub
                let recipient_pk = match PublicKey::parse(&recipient) {
                    Ok(pk) => pk,
                    Err(e) => {
                        eprintln!("Invalid npub '{}': {}", recipient, e);
                        return;
                    }
                };

                println!("Fetching KeyPackage for {}...", recipient);
                let kp_event = match marmot_agent_core::relay::fetch_keypackage(
                    recipient_pk,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(Some(e)) => e,
                    Ok(None) => {
                        eprintln!("No KeyPackage found for {}. Ask them to publish one first.", recipient);
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch KeyPackage: {}", e);
                        return;
                    }
                };

                let relays: Vec<nostr::RelayUrl> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter()
                    .filter_map(|url| nostr::RelayUrl::parse(url).ok())
                    .collect();

                println!("Creating DM group with {}...", recipient);
                let result = match ctx.create_dm(&format!("dm:{}", recipient), relays, &kp_event) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("DM creation failed: {}", e);
                        return;
                    }
                };

                println!("DM group created!");
                println!("  Commit event ID: {}", result.evolution_event.id);
                println!("  Welcome rumors: {}", result.welcome_rumors.as_ref().map(|w| w.len()).unwrap_or(0));

                if publish {
                    println!("\n  Publishing events to relays...");
                    let to_publish = match ctx.prepare_group_update_events(&result) {
                        Ok(e) => e,
                        Err(e) => {
                            eprintln!("Failed to prepare events for publishing: {}", e);
                            return;
                        }
                    };

                    let refs: Vec<(&str, &Event)> = to_publish.iter()
                        .map(|(label, ev)| (*label, ev))
                        .collect();

                    let results = match marmot_agent_core::relay::publish_events(
                        &refs,
                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                    ).await {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("Publish failed: {}", e);
                            return;
                        }
                    };

                    println!("  Publish results:");
                    for (label, relay_results) in results {
                        let ok_count = relay_results.iter().filter(|(_, ok)| *ok).count();
                        println!("    {}: {}/{} relays OK", label, ok_count, relay_results.len());
                        for (url, ok) in relay_results {
                            let status = if ok { "OK" } else { "FAIL" };
                            println!("      {} {}", status, url);
                        }
                    }
                } else {
                    println!("\n  NOTE: Events not published. Use --publish to send them.");
                }
            }
            DmAction::List => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                let ctx = match marmot_agent_core::context::AgentContext::with_default(storage).await {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        eprintln!("No default identity set.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to load agent context: {}", e);
                        return;
                    }
                };

                match ctx.list_groups() {
                    Ok(groups) => {
                        if groups.is_empty() {
                            println!("No conversations found.");
                        } else {
                            println!("Conversations:");
                            for g in groups {
                                let name = if g.name.is_empty() { "unnamed" } else { &g.name };
                                println!("  '{}' (id: {:?})", name, g.mls_group_id);
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to list conversations: {}", e),
                }
            }
            DmAction::Send { group, message, publish } => {
                let storage = match marmot_agent_core::storage::AgentStorage::init().await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to initialize storage: {}", e);
                        return;
                    }
                };

                let ctx = match marmot_agent_core::context::AgentContext::with_default(storage).await {
                    Ok(Some(c)) => c,
                    Ok(None) => {
                        eprintln!("No default identity set.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to load agent context: {}", e);
                        return;
                    }
                };

                // Parse group ID from hex string
                let group_id_bytes = match hex::decode(&group) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Invalid group ID (expected hex): {}", e);
                        return;
                    }
                };
                let group_id = GroupId::from_slice(&group_id_bytes);

                match ctx.create_dm_message(&group_id, &message) {
                    Ok(event) => {
                        println!("Encrypted message created!");
                        println!("  Event ID: {}", event.id);
                        println!("  Kind: {}", event.kind);

                        if publish {
                            println!("  Publishing to relays...");
                            let results = match marmot_agent_core::relay::publish_event(
                                &event,
                                &marmot_agent_core::relay::DEFAULT_RELAYS,
                            ).await {
                                Ok(r) => r,
                                Err(e) => {
                                    eprintln!("Publish failed: {}", e);
                                    return;
                                }
                            };

                            let ok_count = results.iter().filter(|(_, ok)| *ok).count();
                            println!("  Published: {}/{} relays OK", ok_count, results.len());
                            for (url, ok) in results {
                                let status = if ok { "OK" } else { "FAIL" };
                                println!("    {} {}", status, url);
                            }
                        } else {
                            println!("\n  NOTE: Not published. Use --publish to send.");
                        }
                    }
                    Err(e) => eprintln!("Message creation failed: {}", e),
                }
            }
        },
    }
}
