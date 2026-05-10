use clap::{Parser, Subcommand};
use mdk_core::messages::MessageProcessingResult;
use std::sync::Arc;
use tracing::Level;
use nostr::{Event, PublicKey};

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
    /// Fetch and decrypt incoming messages for all known groups.
    Receive {
        #[arg(short, long, help = "Max events to fetch per group", default_value = "50")]
        limit: usize,
        #[arg(long, help = "Do not connect to relays; only show already-stored messages")]
        offline: bool,
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
    /// List all local groups.
    List,
    /// Create a new MLS group.
    Create {
        #[arg(short, long, help = "Group name")]
        name: String,
        #[arg(long, help = "Also publish the group creation events to relays")]
        publish: bool,
    },
    /// Invite a member to a group by fetching their KeyPackage.
    Invite {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Member npub to invite")]
        member: String,
        #[arg(long, help = "Also publish the commit + welcome events to relays")]
        publish: bool,
    },
    /// Show members of a group.
    Members {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
    },
    /// Show stored decrypted messages for a group.
    Messages {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Number of messages to show", default_value = "20")]
        limit: usize,
    },
    /// List pending group invitations (welcome messages not yet accepted).
    Pending,
    /// Accept all pending group invitations and perform a self-update key rotation.
    Join {
        #[arg(long, help = "Publish self-update commit events to relays after joining")]
        publish: bool,
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
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Message content")]
        message: String,
        #[arg(long, help = "Also publish the message to group relays")]
        publish: bool,
    },
    /// Show stored decrypted messages for a DM conversation.
    Messages {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Number of messages to show", default_value = "20")]
        limit: usize,
    },
}

async fn load_storage() -> Option<marmot_agent_core::storage::AgentStorage> {
    match marmot_agent_core::storage::AgentStorage::init().await {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("error: failed to initialize storage: {e}");
            None
        }
    }
}

async fn load_default_context() -> Option<marmot_agent_core::context::AgentContext> {
    let storage = load_storage().await?;
    match marmot_agent_core::context::AgentContext::with_default(storage).await {
        Ok(Some(c)) => Some(c),
        Ok(None) => {
            eprintln!("No default identity set.");
            eprintln!("  Create one:  marmot-cli identity create --name <name>");
            eprintln!("  Set default: marmot-cli identity set-default <name>");
            None
        }
        Err(e) => {
            eprintln!("error: failed to load agent context: {e}");
            None
        }
    }
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
                let Some(storage) = load_storage().await else { return; };

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
                let Some(storage) = load_storage().await else { return; };

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
                let Some(storage) = load_storage().await else { return; };

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
                let Some(storage) = load_storage().await else { return; };

                if let Err(e) = storage.delete_identity(&name).await {
                    eprintln!("Failed to delete identity '{}': {}", name, e);
                } else {
                    println!("Identity '{}' deleted.", name);
                }
            }
            IdentityAction::SetDefault { name } => {
                let Some(mut storage) = load_storage().await else { return; };

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
                let Some(ctx) = load_default_context().await else { return; };

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
                let Some(ctx) = load_default_context().await else { return; };

                let pubkey = ctx.identity.keys.public_key();
                println!("Fetching your KeyPackage from relays...");
                match marmot_agent_core::relay::fetch_keypackage(
                    pubkey,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(Some(event)) => {
                        println!("KeyPackage found:");
                        println!("  npub:      {}", ctx.npub());
                        println!("  event ID:  {}", event.id);
                        println!("  kind:      {}", event.kind);
                        println!("  created:   {}", event.created_at);
                    }
                    Ok(None) => {
                        println!("No KeyPackage found for {}.", ctx.npub());
                        println!("  Publish one with: marmot-cli keypackage publish");
                    }
                    Err(e) => eprintln!("Failed to fetch KeyPackage: {}", e),
                }
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

            match tokio::task::spawn_blocking(move || {
                marmot_agent_rpc::server::serve_tcp_blocking(&listen, handler)
            })
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("Daemon error: {e}"),
                Err(e) => eprintln!("Daemon task panicked: {e}"),
            }
        }
        Commands::Groups { action } => match action {
            GroupAction::List => {
                let Some(ctx) = load_default_context().await else { return; };

                match ctx.list_groups() {
                    Ok(groups) => {
                        if groups.is_empty() {
                            println!("No groups found.");
                        } else {
                            println!("Groups:");
                            for g in groups {
                                let name = if g.name.is_empty() { "unnamed" } else { &g.name };
                                println!("  Group '{}' (nostr-id: {})", name, hex::encode(g.nostr_group_id));
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to list groups: {}", e),
                }
            }
            GroupAction::Create { name, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let relays: Vec<nostr::RelayUrl> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter()
                    .filter_map(|url| nostr::RelayUrl::parse(url).ok())
                    .collect();

                match ctx.create_group(&name, relays) {
                    Ok(result) => {
                        println!("Group '{}' created!", name);
                        println!("  Nostr group ID: {}", hex::encode(result.group.nostr_group_id));

                        if publish {
                            println!("  No welcome events to publish (group has no initial members).");
                            println!("  Use 'groups invite --group <nostr-id> --member <npub> --publish' to add members.");
                        } else {
                            println!("  NOTE: Use --publish to confirm. Add members with 'groups invite'.");
                        }
                    }
                    Err(e) => eprintln!("Group creation failed: {}", e),
                }
            }
            GroupAction::Invite { group, member, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_group = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        eprintln!("Group '{}' not found locally. Use 'groups list' to see available groups.", group);
                        return;
                    }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                let member_pk = match PublicKey::parse(&member) {
                    Ok(pk) => pk,
                    Err(e) => { eprintln!("Invalid npub '{}': {e}", member); return; }
                };

                println!("Fetching KeyPackage for {}...", member);
                let kp_event = match marmot_agent_core::relay::fetch_keypackage(
                    member_pk,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(Some(e)) => e,
                    Ok(None) => {
                        eprintln!("No KeyPackage found for {}. Ask them to run 'keypackage publish' first.", member);
                        return;
                    }
                    Err(e) => { eprintln!("Failed to fetch KeyPackage: {e}"); return; }
                };

                let result = match ctx.invite_member_to_group(&target_group.mls_group_id, &kp_event) {
                    Ok(r) => r,
                    Err(e) => { eprintln!("Invite failed: {e}"); return; }
                };

                println!("Member invited to '{}'!", target_group.name);
                println!("  Commit event ID: {}", result.evolution_event.id);
                println!("  Welcome rumors: {}", result.welcome_rumors.as_ref().map(|w| w.len()).unwrap_or(0));

                if publish {
                    println!("\n  Publishing events to relays...");
                    let commit_ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                    match marmot_agent_core::relay::publish_events(
                        &[("evolution_commit", commit_ev)],
                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                    ).await {
                        Ok(results) => {
                            for (label, relay_results) in results {
                                let ok = relay_results.iter().filter(|(_, ok)| *ok).count();
                                println!("    {}: {}/{} relays OK", label, ok, relay_results.len());
                            }
                        }
                        Err(e) => eprintln!("Publish commit failed: {e}"),
                    }

                    if let Some(rumors) = result.welcome_rumors {
                        for rumor in rumors {
                            match ctx.gift_wrap_welcome(rumor, &member_pk).await {
                                Ok(gift_wrap) => {
                                    match marmot_agent_core::relay::publish_event(
                                        &gift_wrap,
                                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                                    ).await {
                                        Ok(r) => {
                                            let ok = r.iter().filter(|(_, ok)| *ok).count();
                                            println!("    welcome (gift wrap): {}/{} relays OK", ok, r.len());
                                        }
                                        Err(e) => eprintln!("Publish welcome failed: {e}"),
                                    }
                                }
                                Err(e) => eprintln!("Gift-wrap welcome failed: {e}"),
                            }
                        }
                    }
                } else {
                    println!("  NOTE: Use --publish to send commit + welcome events to relays.");
                }
            }
            GroupAction::Members { group } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_group = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        eprintln!("Group '{}' not found.", group);
                        return;
                    }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.get_members_for_group(&target_group.mls_group_id) {
                    Ok(members) => {
                        println!("Members of '{}':", target_group.name);
                        for pk in &members {
                            let npub = marmot_agent_core::context::AgentContext::member_npub(pk);
                            let marker = if pk == &ctx.identity.keys.public_key() { " (you)" } else { "" };
                            println!("  {}{}", npub, marker);
                        }
                        println!("  ({} total)", members.len());
                    }
                    Err(e) => eprintln!("Failed to get members: {e}"),
                }
            }
            GroupAction::Messages { group, limit } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_group = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        eprintln!("Group '{}' not found. Use 'groups list' to see available groups.", group);
                        return;
                    }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.get_messages_for_group(&target_group.mls_group_id, limit) {
                    Ok(messages) => {
                        if messages.is_empty() {
                            println!("No messages in '{}'. Run 'receive' to fetch from relays.", target_group.name);
                        } else {
                            println!("Messages in '{}' (newest first):", target_group.name);
                            for msg in &messages {
                                let sender = marmot_agent_core::context::AgentContext::member_npub(&msg.pubkey);
                                println!("  [{}] {}: {}", msg.created_at, &sender[..16], msg.content);
                            }
                            println!("  ({} messages)", messages.len());
                        }
                    }
                    Err(e) => eprintln!("Failed to get messages: {e}"),
                }
            }
            GroupAction::Pending => {
                let Some(ctx) = load_default_context().await else { return; };

                match ctx.list_pending_welcomes() {
                    Ok(welcomes) => {
                        if welcomes.is_empty() {
                            println!("No pending group invitations.");
                            println!("  Run 'receive' to fetch new invitations from relays.");
                        } else {
                            println!("Pending group invitations ({}):", welcomes.len());
                            for (i, w) in welcomes.iter().enumerate() {
                                println!("  [{}] nostr group: {}", i + 1, hex::encode(w.nostr_group_id));
                            }
                            println!("\nRun 'groups join' to accept all pending invitations.");
                        }
                    }
                    Err(e) => eprintln!("Failed to list pending welcomes: {e}"),
                }
            }
            GroupAction::Join { publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let welcomes = match ctx.list_pending_welcomes() {
                    Ok(w) => w,
                    Err(e) => { eprintln!("Failed to list pending welcomes: {e}"); return; }
                };

                if welcomes.is_empty() {
                    println!("No pending group invitations to accept.");
                    println!("  Run 'receive' to fetch new invitations from relays.");
                    return;
                }

                println!("Accepting {} pending invitation(s)...", welcomes.len());
                let mut accepted = 0usize;
                for welcome in &welcomes {
                    let nostr_id = hex::encode(welcome.nostr_group_id);
                    match ctx.accept_welcome(welcome) {
                        Ok(()) => {
                            println!("  Joined group: {}", nostr_id);
                            accepted += 1;
                        }
                        Err(e) => eprintln!("  Failed to join {}: {e}", nostr_id),
                    }
                }

                if accepted == 0 {
                    return;
                }

                // Run self-update on all groups that need a key rotation after joining.
                let needing_update = match ctx.groups_needing_self_update(0) {
                    Ok(ids) => ids,
                    Err(e) => {
                        eprintln!("Failed to check groups needing self-update: {e}");
                        return;
                    }
                };

                if needing_update.is_empty() {
                    println!("\nNo self-update needed.");
                    return;
                }

                println!("\nRunning self-update for {} group(s)...", needing_update.len());
                for group_id in &needing_update {
                    match ctx.self_update_group(group_id) {
                        Ok(result) => {
                            println!("  Self-update commit: {}", result.evolution_event.id);

                            if publish {
                                let ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                                match marmot_agent_core::relay::publish_events(
                                    &[("evolution_commit", ev)],
                                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                                ).await {
                                    Ok(results) => {
                                        for (label, relay_results) in results {
                                            let ok = relay_results.iter().filter(|(_, ok)| *ok).count();
                                            println!("    {}: {}/{} relays OK", label, ok, relay_results.len());
                                        }
                                    }
                                    Err(e) => eprintln!("  Publish failed: {e}"),
                                }
                            }
                        }
                        Err(e) => eprintln!("  Self-update failed: {e}"),
                    }
                }

                if !publish {
                    println!("  NOTE: Use --publish to send self-update commits to relays.");
                }
            }
        },
        Commands::Dm { action } => match action {
            DmAction::Create { recipient, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

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

                    // Publish the MLS commit event
                    let commit_ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                    match marmot_agent_core::relay::publish_events(
                        &[("evolution_commit", commit_ev)],
                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                    ).await {
                        Ok(results) => {
                            for (label, relay_results) in results {
                                let ok = relay_results.iter().filter(|(_, ok)| *ok).count();
                                println!("    {}: {}/{} relays OK", label, ok, relay_results.len());
                            }
                        }
                        Err(e) => eprintln!("Publish commit failed: {}", e),
                    }

                    // Gift-wrap each welcome rumor for the recipient (NIP-59 kind 1059)
                    if let Some(rumors) = result.welcome_rumors {
                        for rumor in rumors {
                            match ctx.gift_wrap_welcome(rumor, &recipient_pk).await {
                                Ok(gift_wrap) => {
                                    match marmot_agent_core::relay::publish_event(
                                        &gift_wrap,
                                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                                    ).await {
                                        Ok(r) => {
                                            let ok = r.iter().filter(|(_, ok)| *ok).count();
                                            println!("    welcome (gift wrap): {}/{} relays OK", ok, r.len());
                                        }
                                        Err(e) => eprintln!("Publish welcome failed: {}", e),
                                    }
                                }
                                Err(e) => eprintln!("Gift-wrap welcome failed: {}", e),
                            }
                        }
                    }
                } else {
                    println!("\n  NOTE: Events not published. Use --publish to send them.");
                }
            }
            DmAction::List => {
                let Some(ctx) = load_default_context().await else { return; };

                match ctx.list_groups() {
                    Ok(groups) => {
                        if groups.is_empty() {
                            println!("No conversations found.");
                        } else {
                            println!("Conversations:");
                            for g in groups {
                                let name = if g.name.is_empty() { "unnamed" } else { &g.name };
                                println!("  '{}' (nostr-id: {})", name, hex::encode(g.nostr_group_id));
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to list conversations: {}", e),
                }
            }
            DmAction::Send { group, message, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => {
                        match ctx.create_dm_message(&g.mls_group_id, &message) {
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
                    Ok(None) => {
                        eprintln!("Group with nostr id '{}' not found locally. Run 'dm list' to see available groups.", group);
                    }
                    Err(e) => eprintln!("Failed to find group: {}", e),
                }
            }
            DmAction::Messages { group, limit } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_group = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        eprintln!("Conversation '{}' not found. Use 'dm list' to see available conversations.", group);
                        return;
                    }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.get_messages_for_group(&target_group.mls_group_id, limit) {
                    Ok(messages) => {
                        if messages.is_empty() {
                            println!("No messages in '{}'. Run 'receive' to fetch from relays.", target_group.name);
                        } else {
                            println!("Messages in '{}' (newest first):", target_group.name);
                            for msg in &messages {
                                let sender = marmot_agent_core::context::AgentContext::member_npub(&msg.pubkey);
                                println!("  [{}] {}: {}", msg.created_at, &sender[..16], msg.content);
                            }
                            println!("  ({} messages)", messages.len());
                        }
                    }
                    Err(e) => eprintln!("Failed to get messages: {e}"),
                }
            }
        },
        Commands::Receive { limit, offline } => {
            let Some(ctx) = load_default_context().await else { return; };

            let groups = match ctx.list_groups() {
                Ok(g) => g,
                Err(e) => { eprintln!("Failed to list groups: {e}"); return; }
            };

            // Collect h-tags (nostr group IDs) for all known groups
            let h_tags: Vec<String> = groups.iter()
                .map(|g| hex::encode(g.nostr_group_id))
                .collect();

            if !h_tags.is_empty() {
                println!("Checking {} known group(s)...", groups.len());
            }

            let group_events = if offline {
                println!("  (offline mode — skipping relay fetch)");
                vec![]
            } else if h_tags.is_empty() {
                vec![]
            } else {
                match marmot_agent_core::relay::fetch_group_events(
                    &h_tags,
                    limit,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(evs) => {
                        println!("  Fetched {} group event(s) from relays.", evs.len());
                        evs
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch group events: {e}");
                        return;
                    }
                }
            };

            let mut new_messages = 0usize;
            let mut commits = 0usize;
            let mut skipped = 0usize;

            for event in &group_events {
                match ctx.process_incoming_event(event) {
                    Ok(MessageProcessingResult::ApplicationMessage(_)) => {
                        new_messages += 1;
                    }
                    Ok(MessageProcessingResult::Commit { .. }) => {
                        commits += 1;
                    }
                    Ok(_) => {
                        skipped += 1;
                    }
                    Err(e) => {
                        tracing::warn!("failed to process event {}: {}", event.id, e);
                        skipped += 1;
                    }
                }
            }

            // Fetch gift-wrap events (kind 1059) addressed to us — these carry welcome invites.
            let gift_wrap_events = if offline {
                vec![]
            } else {
                println!("Checking for group invitations (gift wraps)...");
                match marmot_agent_core::relay::fetch_gift_wrap_events(
                    ctx.identity.keys.public_key(),
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(evs) => {
                        println!("  Fetched {} gift-wrap event(s) from relays.", evs.len());
                        evs
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch gift-wrap events: {e}");
                        vec![]
                    }
                }
            };

            let mut new_welcomes = 0usize;
            for gift_wrap in &gift_wrap_events {
                match ctx.unwrap_and_process_welcome(gift_wrap).await {
                    Ok(Some(_)) => new_welcomes += 1,
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!("failed to process gift wrap {}: {}", gift_wrap.id, e);
                    }
                }
            }

            println!("Done.");
            println!("  {} new message(s)", new_messages);
            if commits > 0 { println!("  {} MLS commit(s) applied", commits); }
            if skipped > 0 { println!("  {} event(s) skipped (already processed or unrecognised)", skipped); }
            if new_welcomes > 0 {
                println!("  {} new group invitation(s) received", new_welcomes);
                println!("\nRun 'groups pending' to review, then 'groups join' to accept.");
            }
            if new_messages > 0 {
                println!("\nRun 'groups messages --group <id>' or 'dm messages --group <id>' to read.");
            }
        }
    }
}
