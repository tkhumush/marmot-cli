use clap::{Parser, Subcommand};
use mdk_core::messages::MessageProcessingResult;
use std::sync::Arc;
use tracing::Level;
use nostr::{Event, EventId, PublicKey};

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
    /// Message reactions, deletions, and search within groups.
    Messages {
        #[command(subcommand)]
        action: MessageAction,
    },
    /// Manage your Nostr profile (kind 0 metadata).
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Look up a user's profile by npub.
    Users {
        #[command(subcommand)]
        action: UsersAction,
    },
    /// Manage your follow list (kind 3 contact list).
    Follows {
        #[command(subcommand)]
        action: FollowsAction,
    },
    /// Unified view of all conversations (DMs + groups).
    Chats {
        #[command(subcommand)]
        action: ChatsAction,
    },
    /// Fetch and decrypt incoming messages for all known groups.
    Receive {
        #[arg(short, long, help = "Max events to fetch per group", default_value = "50")]
        limit: usize,
        #[arg(long, help = "Do not connect to relays; only show already-stored messages")]
        offline: bool,
    },
    /// Diagnose relay events for a given pubkey (interop debugging).
    Debug {
        #[arg(help = "Pubkey (hex or npub) to inspect events for")]
        pubkey: String,
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
    /// Show all configured relay categories.
    List,
    /// Add a relay to your inbox list (kind 10050) and republish.
    Add {
        #[arg(help = "Relay WebSocket URL (wss://...)")]
        url: String,
    },
    /// Remove a relay from your inbox list (kind 10050) and republish.
    Remove {
        #[arg(help = "Relay WebSocket URL to remove")]
        url: String,
    },
}

#[derive(Subcommand)]
enum KeypackageAction {
    Publish,
    Show,
    /// List all our key packages currently on relays.
    List,
    /// Delete a specific key package event from relays (kind 5 deletion).
    Delete {
        #[arg(help = "Event ID of the key package to delete (hex)")]
        event_id: String,
    },
    /// Delete ALL our key packages from relays. Requires --confirm.
    DeleteAll {
        #[arg(long, help = "Confirm deletion of all key packages")]
        confirm: bool,
    },
    /// Check if a given user has a valid key package on relays.
    Check {
        #[arg(help = "npub or hex pubkey of the user to check")]
        npub: String,
    },
}

#[derive(Subcommand)]
enum GroupAction {
    /// List all local groups.
    List,
    /// Show full metadata for a group.
    Show {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
    },
    /// Create a new MLS group.
    Create {
        #[arg(short, long, help = "Group name")]
        name: String,
        #[arg(short, long, help = "Group description", default_value = "")]
        description: String,
        #[arg(long = "member", help = "Member npub to invite on creation (repeat for multiple)", action = clap::ArgAction::Append)]
        members: Vec<String>,
        #[arg(long, help = "Also publish the group creation events to relays")]
        publish: bool,
    },
    /// Send a message to a group.
    Send {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Message content")]
        message: String,
        #[arg(long, help = "Event ID of message to reply to (hex)")]
        reply_to: Option<String>,
        #[arg(long, help = "Publish the message to relays")]
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
        #[arg(long, help = "Only show messages before this Unix timestamp")]
        before: Option<u64>,
        #[arg(long, help = "Only show messages after this Unix timestamp")]
        after: Option<u64>,
    },
    /// List pending group invitations (welcome messages not yet accepted).
    Pending,
    /// Accept all pending group invitations.
    Join {
        #[arg(long, help = "Publish self-update commit events to relays after joining")]
        publish: bool,
    },
    /// Accept a specific pending invitation by nostr group ID.
    Accept {
        #[arg(short, long, help = "Group nostr ID (hex h-tag) of the pending invitation")]
        group: String,
    },
    /// Decline a specific pending invitation by nostr group ID.
    Decline {
        #[arg(short, long, help = "Group nostr ID (hex h-tag) of the pending invitation")]
        group: String,
    },
    /// Rename a group (admin only).
    Rename {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "New group name")]
        name: String,
        #[arg(long, help = "Publish the commit event to relays")]
        publish: bool,
    },
    /// Remove one or more members from a group (admin only).
    RemoveMembers {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(long = "member", help = "Member npub to remove (repeat for multiple)", action = clap::ArgAction::Append)]
        members: Vec<String>,
        #[arg(long, help = "Publish the commit event to relays")]
        publish: bool,
    },
    /// Promote a member to admin (admin only).
    Promote {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Member npub to promote to admin")]
        member: String,
        #[arg(long, help = "Publish the commit event to relays")]
        publish: bool,
    },
    /// Demote an admin to member (admin only). Fails if they are the last admin.
    Demote {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Admin npub to demote")]
        member: String,
        #[arg(long, help = "Publish the commit event to relays")]
        publish: bool,
    },
    /// Remove yourself from the admin list. Required before leaving if you are an admin.
    SelfDemote {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(long, help = "Publish the commit event to relays")]
        publish: bool,
    },
    /// Leave a group. Run 'self-demote' first if you are an admin.
    Leave {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(long, help = "Publish the leave event to relays")]
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
        #[arg(long, help = "Event ID of message to reply to (hex)")]
        reply_to: Option<String>,
        #[arg(long, help = "Also publish the message to group relays")]
        publish: bool,
    },
    /// Show stored decrypted messages for a DM conversation.
    Messages {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Number of messages to show", default_value = "20")]
        limit: usize,
        #[arg(long, help = "Only show messages before this Unix timestamp")]
        before: Option<u64>,
        #[arg(long, help = "Only show messages after this Unix timestamp")]
        after: Option<u64>,
    },
}

#[derive(Subcommand)]
enum MessageAction {
    /// Send an emoji reaction to a message inside a group.
    React {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Event ID of the message to react to (hex)")]
        event_id: String,
        #[arg(short, long, help = "Emoji to react with", default_value = "+")]
        emoji: String,
        #[arg(long, help = "Publish to relays")]
        publish: bool,
    },
    /// Request deletion of a message inside a group (kind 5).
    Delete {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(short, long, help = "Event ID of the message to delete (hex)")]
        event_id: String,
        #[arg(long, help = "Publish to relays")]
        publish: bool,
    },
    /// Search stored messages in a specific group (case-insensitive substring).
    Search {
        #[arg(short, long, help = "Group nostr ID (hex h-tag)")]
        group: String,
        #[arg(help = "Search query (case-insensitive substring)")]
        query: String,
        #[arg(short, long, help = "Max results to show", default_value = "50")]
        limit: usize,
    },
    /// Search stored messages across all groups (case-insensitive substring).
    SearchAll {
        #[arg(help = "Search query (case-insensitive substring)")]
        query: String,
        #[arg(short, long, help = "Max results per group", default_value = "20")]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Fetch and display your Nostr profile (kind 0) from relays.
    Show,
    /// Update your Nostr profile and publish kind 0 to relays.
    Update {
        #[arg(long, help = "Short username")]
        name: Option<String>,
        #[arg(long, help = "Display name")]
        display_name: Option<String>,
        #[arg(long, help = "About/bio")]
        about: Option<String>,
        #[arg(long, help = "Avatar URL")]
        picture: Option<String>,
        #[arg(long, help = "NIP-05 identifier (user@domain.com)")]
        nip05: Option<String>,
        #[arg(long, help = "Lightning address (user@domain.com)")]
        lud16: Option<String>,
    },
}

#[derive(Subcommand)]
enum UsersAction {
    /// Fetch and display a user's Nostr profile from relays.
    Show {
        #[arg(help = "npub or hex pubkey")]
        npub: String,
    },
}

#[derive(Subcommand)]
enum FollowsAction {
    /// List accounts you follow (kind 3 contact list).
    List,
    /// Follow a user — adds them to your kind 3 contact list.
    Add {
        #[arg(help = "npub or hex pubkey to follow")]
        npub: String,
    },
    /// Unfollow a user — removes them from your kind 3 contact list.
    Remove {
        #[arg(help = "npub or hex pubkey to unfollow")]
        npub: String,
    },
}

#[derive(Subcommand)]
enum ChatsAction {
    /// Unified view of all conversations with last-message preview.
    List {
        #[arg(short, long, help = "Max conversations to show", default_value = "50")]
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

/// Publish a message event to the group's relays + default relays, print relay results.
async fn publish_message_event(
    event: &nostr::Event,
    group_relays: Option<Vec<String>>,
) {
    let mut relay_urls: Vec<String> = marmot_agent_core::relay::DEFAULT_RELAYS
        .iter().map(|s| s.to_string()).collect();
    if let Some(extra) = group_relays {
        for r in extra {
            if !relay_urls.contains(&r) { relay_urls.push(r); }
        }
    }
    let relay_refs: Vec<&str> = relay_urls.iter().map(|s| s.as_str()).collect();
    println!("  Publishing to relays...");
    match marmot_agent_core::relay::publish_event(event, &relay_refs).await {
        Ok(results) => {
            let ok = results.iter().filter(|(_, ok)| *ok).count();
            println!("  Published: {}/{} relays OK", ok, results.len());
            for (url, ok) in results {
                println!("    {} {}", if ok { "OK" } else { "FAIL" }, url);
            }
        }
        Err(e) => eprintln!("  Publish failed: {e}"),
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
                println!("Default relays (built-in):");
                for url in marmot_agent_core::relay::DEFAULT_RELAYS {
                    println!("  {}", url);
                }

                // Show inbox relays (kind 10050) if identity is set
                if let Some(ctx) = load_default_context().await {
                    let our_pk = ctx.identity.keys.public_key();

                    let inbox = marmot_agent_core::relay::fetch_inbox_relays(
                        our_pk,
                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                    ).await;
                    println!("\nInbox relays (kind 10050 — where others send gift-wraps to you):");
                    if inbox.is_empty() {
                        println!("  (none published — run 'keypackage publish' to set them)");
                    } else {
                        for r in &inbox {
                            println!("  {}", r);
                        }
                    }

                    // Show NIP-65 relay list (kind 10002)
                    let filter = nostr::Filter::new()
                        .kind(nostr::Kind::RelayList)
                        .author(our_pk)
                        .limit(1);
                    if let Ok(events) = marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                        println!("\nNIP-65 relay list (kind 10002):");
                        if let Some(ev) = events.into_iter().next() {
                            for tag in ev.tags.iter() {
                                if tag.kind() == nostr::TagKind::SingleLetter(nostr::SingleLetterTag::lowercase(nostr::Alphabet::R)) {
                                    let parts: Vec<_> = tag.as_slice().iter().skip(1).collect();
                                    println!("  {}", parts.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "));
                                }
                            }
                        } else {
                            println!("  (none published)");
                        }
                    }
                }
            }
            RelayAction::Add { url } => {
                let Some(ctx) = load_default_context().await else { return; };

                let our_pk = ctx.identity.keys.public_key();
                let existing_inbox = marmot_agent_core::relay::fetch_inbox_relays(
                    our_pk,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await;

                let mut relays = existing_inbox;
                if relays.contains(&url) {
                    println!("Relay '{}' is already in your inbox list.", url);
                    return;
                }
                relays.push(url.clone());

                let tags: Vec<nostr::Tag> = relays.iter()
                    .filter_map(|r| nostr::RelayUrl::parse(r).ok())
                    .map(nostr::Tag::relay)
                    .collect();

                let event = match nostr::EventBuilder::new(nostr::Kind::Custom(10050), "")
                    .tags(tags)
                    .sign_with_keys(&ctx.identity.keys)
                {
                    Ok(e) => e,
                    Err(e) => { eprintln!("Failed to build relay list event: {e}"); return; }
                };

                match marmot_agent_core::relay::publish_event(&event, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(results) => {
                        let ok = results.iter().filter(|(_, ok)| *ok).count();
                        println!("Relay '{}' added to inbox list: {}/{} relays OK", url, ok, results.len());
                    }
                    Err(e) => eprintln!("Publish failed: {e}"),
                }
            }
            RelayAction::Remove { url } => {
                let Some(ctx) = load_default_context().await else { return; };

                let our_pk = ctx.identity.keys.public_key();
                let existing_inbox = marmot_agent_core::relay::fetch_inbox_relays(
                    our_pk,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await;

                let before_len = existing_inbox.len();
                let relays: Vec<String> = existing_inbox.into_iter().filter(|r| r != &url).collect();

                if relays.len() == before_len {
                    println!("Relay '{}' was not in your inbox list.", url);
                    return;
                }

                let tags: Vec<nostr::Tag> = relays.iter()
                    .filter_map(|r| nostr::RelayUrl::parse(r).ok())
                    .map(nostr::Tag::relay)
                    .collect();

                let event = match nostr::EventBuilder::new(nostr::Kind::Custom(10050), "")
                    .tags(tags)
                    .sign_with_keys(&ctx.identity.keys)
                {
                    Ok(e) => e,
                    Err(e) => { eprintln!("Failed to build relay list event: {e}"); return; }
                };

                match marmot_agent_core::relay::publish_event(&event, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(results) => {
                        let ok = results.iter().filter(|(_, ok)| *ok).count();
                        println!("Relay '{}' removed from inbox list: {}/{} relays OK", url, ok, results.len());
                    }
                    Err(e) => eprintln!("Publish failed: {e}"),
                }
            }
        },
        Commands::Keypackage { action } => match action {
            KeypackageAction::Publish => {
                let Some(ctx) = load_default_context().await else { return; };

                let relays: Vec<nostr::RelayUrl> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter()
                    .filter_map(|url| nostr::RelayUrl::parse(url).ok())
                    .collect();

                let kp_data = match ctx.create_keypackage(relays.clone()) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("KeyPackage creation failed: {}", e);
                        return;
                    }
                };

                let event = match ctx.sign_keypackage_event(&kp_data) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("Event signing failed: {}", e);
                        return;
                    }
                };

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

                // Also publish inbox relay list (kind 10050) if not already set.
                // This is required so others know where to send gift-wrap welcome events.
                let our_pk = ctx.identity.keys.public_key();
                let existing_inbox = marmot_agent_core::relay::fetch_inbox_relays(
                    our_pk,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await;
                if existing_inbox.is_empty() {
                    println!("\nPublishing inbox relay list (kind 10050)...");
                    let relay_tags: Vec<nostr::Tag> = marmot_agent_core::relay::DEFAULT_RELAYS.iter()
                        .filter_map(|u| nostr::RelayUrl::parse(u).ok())
                        .map(nostr::Tag::relay)
                        .collect();
                    // kind 10050: NIP-17 inbox relays (where gift-wraps are sent to you)
                    if let Ok(inbox_event) = nostr::EventBuilder::new(nostr::Kind::Custom(10050), "")
                        .tags(relay_tags.clone())
                        .sign_with_keys(&ctx.identity.keys)
                    {
                        match marmot_agent_core::relay::publish_event(
                            &inbox_event,
                            &marmot_agent_core::relay::DEFAULT_RELAYS,
                        ).await {
                            Ok(r) => {
                                let ok = r.iter().filter(|(_, ok)| *ok).count();
                                println!("  Inbox relay list (10050) published: {}/{} relays OK", ok, r.len());
                            }
                            Err(e) => eprintln!("  Inbox relay list publish failed: {e}"),
                        }
                    }
                    // kind 10051: Marmot key package relay list (where to fetch KeyPackages)
                    if let Ok(kp_relay_event) = nostr::EventBuilder::new(nostr::Kind::Custom(10051), "")
                        .tags(relay_tags)
                        .sign_with_keys(&ctx.identity.keys)
                    {
                        match marmot_agent_core::relay::publish_event(
                            &kp_relay_event,
                            &marmot_agent_core::relay::DEFAULT_RELAYS,
                        ).await {
                            Ok(r) => {
                                let ok = r.iter().filter(|(_, ok)| *ok).count();
                                println!("  Key package relay list (10051) published: {}/{} relays OK", ok, r.len());
                            }
                            Err(e) => eprintln!("  Key package relay list publish failed: {e}"),
                        }
                    }
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
            KeypackageAction::Check { npub } => {
                let target_pk = match PublicKey::parse(&npub) {
                    Ok(pk) => pk,
                    Err(e) => { eprintln!("Invalid npub '{}': {e}", npub); return; }
                };
                println!("Checking KeyPackage for {}...", npub);
                match marmot_agent_core::relay::fetch_keypackage(
                    target_pk,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(Some(event)) => {
                        println!("KeyPackage found — user is reachable.");
                        println!("  event ID: {}", event.id);
                        println!("  created:  {}", event.created_at);
                    }
                    Ok(None) => {
                        println!("No KeyPackage found for {}.", npub);
                        println!("  They need to run 'keypackage publish' before you can DM or invite them.");
                    }
                    Err(e) => eprintln!("Failed to check KeyPackage: {e}"),
                }
            }
            KeypackageAction::List => {
                let Some(ctx) = load_default_context().await else { return; };

                let filter = nostr::Filter::new()
                    .kind(nostr::Kind::Custom(30443))
                    .author(ctx.identity.keys.public_key())
                    .limit(50);
                println!("Fetching your KeyPackages from relays...");
                match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(events) => {
                        if events.is_empty() {
                            println!("No KeyPackages found on relays.");
                            println!("  Publish one with: marmot-cli keypackage publish");
                        } else {
                            println!("KeyPackages on relays ({}):", events.len());
                            for ev in &events {
                                println!("  {} (created: {})", ev.id, ev.created_at);
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to fetch KeyPackages: {e}"),
                }
            }
            KeypackageAction::Delete { event_id } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_id = match nostr::EventId::parse(&event_id) {
                    Ok(id) => id,
                    Err(e) => { eprintln!("Invalid event ID '{}': {e}", event_id); return; }
                };

                let delete_event = match nostr::EventBuilder::new(nostr::Kind::EventDeletion, "")
                    .tag(nostr::Tag::event(target_id))
                    .sign_with_keys(&ctx.identity.keys)
                {
                    Ok(e) => e,
                    Err(e) => { eprintln!("Failed to build deletion event: {e}"); return; }
                };

                match marmot_agent_core::relay::publish_event(
                    &delete_event,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await {
                    Ok(results) => {
                        let ok = results.iter().filter(|(_, ok)| *ok).count();
                        println!("Deletion published: {}/{} relays OK", ok, results.len());
                    }
                    Err(e) => eprintln!("Publish failed: {e}"),
                }
            }
            KeypackageAction::DeleteAll { confirm } => {
                let Some(ctx) = load_default_context().await else { return; };

                if !confirm {
                    eprintln!("This will delete ALL your KeyPackages from relays.");
                    eprintln!("  Others will not be able to DM or invite you until you republish.");
                    eprintln!("  Re-run with --confirm to proceed.");
                    return;
                }

                let filter = nostr::Filter::new()
                    .kind(nostr::Kind::Custom(30443))
                    .author(ctx.identity.keys.public_key())
                    .limit(100);
                println!("Fetching your KeyPackages from relays...");
                let events = match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(evs) => evs,
                    Err(e) => { eprintln!("Failed to fetch KeyPackages: {e}"); return; }
                };

                if events.is_empty() {
                    println!("No KeyPackages found to delete.");
                    return;
                }

                println!("Deleting {} KeyPackage(s)...", events.len());
                for ev in &events {
                    let delete_event = match nostr::EventBuilder::new(nostr::Kind::EventDeletion, "")
                        .tag(nostr::Tag::event(ev.id))
                        .sign_with_keys(&ctx.identity.keys)
                    {
                        Ok(e) => e,
                        Err(e) => { eprintln!("  Failed to build deletion for {}: {e}", ev.id); continue; }
                    };
                    match marmot_agent_core::relay::publish_event(
                        &delete_event,
                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                    ).await {
                        Ok(results) => {
                            let ok = results.iter().filter(|(_, ok)| *ok).count();
                            println!("  {} deleted: {}/{} relays OK", ev.id, ok, results.len());
                        }
                        Err(e) => eprintln!("  Failed to delete {}: {e}", ev.id),
                    }
                }
                println!("\nDone. Run 'keypackage publish' to republish a fresh one.");
            }
        },
        Commands::Daemon { listen } => {
            println!("Starting daemon on {}...", listen);

            let handler: marmot_agent_rpc::server::HandlerFn = Arc::new(
                move |method: String, params: serde_json::Value| {
                    match method.as_str() {
                        "ping" => Ok(serde_json::json!({"pong": true})),
                        "identity_npub" => {
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
            GroupAction::Show { group } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_group = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                let members = ctx.get_members_for_group(&target_group.mls_group_id).unwrap_or_default();
                let relays = ctx.get_group_relays(&target_group.mls_group_id).unwrap_or_default();

                println!("Group '{}'", if target_group.name.is_empty() { "<Direct Message>" } else { &target_group.name });
                println!("  nostr-id:    {}", hex::encode(target_group.nostr_group_id));
                println!("  members:     {}", members.len());
                for pk in &members {
                    let npub = marmot_agent_core::context::AgentContext::member_npub(pk);
                    let is_admin = target_group.admin_pubkeys.contains(pk);
                    let is_us = pk == &ctx.identity.keys.public_key();
                    let mut markers = vec![];
                    if is_admin { markers.push("admin"); }
                    if is_us { markers.push("you"); }
                    if markers.is_empty() {
                        println!("    {}", npub);
                    } else {
                        println!("    {} ({})", npub, markers.join(", "));
                    }
                }
                println!("  relays:      {}", relays.len());
                for r in &relays {
                    println!("    {}", r);
                }
            }
            GroupAction::Create { name, description, members, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let relays: Vec<nostr::RelayUrl> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter()
                    .filter_map(|url| nostr::RelayUrl::parse(url).ok())
                    .collect();

                let group_result = match ctx.create_group(&name, &description, relays) {
                    Ok(r) => r,
                    Err(e) => { eprintln!("Group creation failed: {e}"); return; }
                };
                let nostr_id_hex = hex::encode(group_result.group.nostr_group_id);
                println!("Group '{}' created!", name);
                println!("  Nostr group ID: {}", nostr_id_hex);

                if members.is_empty() {
                    if !publish {
                        println!("  NOTE: Add members with 'groups invite --group {} --member <npub> --publish'.", nostr_id_hex);
                    }
                } else {
                    // Fetch all key packages first, skip members that don't have one
                    let mut kp_events: Vec<nostr::Event> = vec![];
                    let mut member_pks: Vec<PublicKey> = vec![];
                    for m in &members {
                        let pk = match PublicKey::parse(m) {
                            Ok(pk) => pk,
                            Err(e) => { eprintln!("  Invalid npub '{}': {e} — skipping.", m); continue; }
                        };
                        println!("  Fetching KeyPackage for {}...", m);
                        match marmot_agent_core::relay::fetch_keypackage(pk, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                            Ok(Some(kp)) => { kp_events.push(kp); member_pks.push(pk); }
                            Ok(None) => eprintln!("  No KeyPackage for {} — skipping.", m),
                            Err(e) => eprintln!("  Failed to fetch KeyPackage for {}: {e} — skipping.", m),
                        }
                    }

                    if !kp_events.is_empty() {
                        match ctx.invite_members_to_group(&group_result.group.mls_group_id, &kp_events) {
                            Ok(update) => {
                                println!("  Added {} member(s).", member_pks.len());
                                if publish {
                                    println!("  Publishing...");
                                    let ev = marmot_agent_core::context::AgentContext::evolution_event(&update);
                                    match marmot_agent_core::relay::publish_event(ev, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                        Ok(r) => {
                                            let ok = r.iter().filter(|(_, ok)| *ok).count();
                                            println!("  commit: {}/{} relays OK", ok, r.len());
                                        }
                                        Err(e) => eprintln!("  Publish commit failed: {e}"),
                                    }

                                    // Send welcome to each member
                                    if let Some(rumors) = update.welcome_rumors {
                                        for (rumor, pk) in rumors.into_iter().zip(member_pks.iter()) {
                                            let inbox = marmot_agent_core::relay::fetch_inbox_relays(
                                                *pk,
                                                &marmot_agent_core::relay::DEFAULT_RELAYS,
                                            ).await;
                                            let welcome_relays: Vec<String> = if inbox.is_empty() {
                                                marmot_agent_core::relay::DEFAULT_RELAYS.iter().map(|s| s.to_string()).collect()
                                            } else {
                                                inbox
                                            };
                                            match ctx.gift_wrap_welcome(rumor, pk).await {
                                                Ok(gift_wrap) => {
                                                    match marmot_agent_core::relay::publish_gift_wrap(
                                                        &gift_wrap,
                                                        &welcome_relays,
                                                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                                                        &ctx.identity.keys,
                                                    ).await {
                                                        Ok(r) => {
                                                            let ok = r.iter().filter(|(_, ok)| *ok).count();
                                                            println!("  welcome → {}: {}/{} relays OK",
                                                                marmot_agent_core::context::AgentContext::member_npub(pk),
                                                                ok, r.len());
                                                        }
                                                        Err(e) => eprintln!("  welcome publish failed: {e}"),
                                                    }
                                                }
                                                Err(e) => eprintln!("  gift-wrap failed: {e}"),
                                            }
                                        }
                                    }
                                } else {
                                    println!("  NOTE: Use --publish to send commit + welcome events to relays.");
                                }
                            }
                            Err(e) => eprintln!("  Failed to add members: {e}"),
                        }
                    }
                }
            }
            GroupAction::Send { group, message, reply_to, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let reply_to_id = match parse_optional_event_id(reply_to.as_deref()) {
                    Ok(id) => id,
                    Err(e) => { eprintln!("{e}"); return; }
                };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found. Use 'groups list'.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.create_message(&g.mls_group_id, &message, reply_to_id) {
                    Ok(event) => {
                        println!("Message created!");
                        println!("  Event ID: {}", event.id);
                        if publish {
                            let relays = ctx.get_group_relays(&g.mls_group_id).ok();
                            publish_message_event(&event, relays).await;
                        } else {
                            println!("  NOTE: Use --publish to send to relays.");
                        }
                    }
                    Err(e) => eprintln!("Message creation failed: {e}"),
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
                        let inbox = marmot_agent_core::relay::fetch_inbox_relays(
                            member_pk,
                            &marmot_agent_core::relay::DEFAULT_RELAYS,
                        ).await;
                        let welcome_relay_strings: Vec<String> = if inbox.is_empty() {
                            marmot_agent_core::relay::DEFAULT_RELAYS.iter().map(|s| s.to_string()).collect()
                        } else {
                            println!("    member inbox relays: {:?}", inbox);
                            inbox
                        };
                        for rumor in rumors {
                            match ctx.gift_wrap_welcome(rumor, &member_pk).await {
                                Ok(gift_wrap) => {
                                    match marmot_agent_core::relay::publish_gift_wrap(
                                        &gift_wrap,
                                        &welcome_relay_strings,
                                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                                        &ctx.identity.keys,
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
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
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
            GroupAction::Messages { group, limit, before, after } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_group = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        eprintln!("Group '{}' not found. Use 'groups list' to see available groups.", group);
                        return;
                    }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.get_messages_for_group(&target_group.mls_group_id, if before.is_some() || after.is_some() { 10_000 } else { limit }) {
                    Ok(mut messages) => {
                        if let Some(ts) = before {
                            messages.retain(|m| m.created_at.as_secs() < ts);
                        }
                        if let Some(ts) = after {
                            messages.retain(|m| m.created_at.as_secs() > ts);
                        }
                        let messages: Vec<_> = messages.into_iter().take(limit).collect();
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
                            println!("\nRun 'groups join' to accept all, or 'groups accept --group <nostr-id>' for one.");
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

                println!("\nAccepted {} invitation(s).", accepted);

                // Rotate KeyPackage so the consumed one is replaced on relays.
                let relays: Vec<nostr::RelayUrl> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter()
                    .filter_map(|url| nostr::RelayUrl::parse(url).ok())
                    .collect();
                match ctx.create_keypackage(relays) {
                    Ok(kp_data) => match ctx.sign_keypackage_event(&kp_data) {
                        Ok(event) => {
                            match marmot_agent_core::relay::publish_event(
                                &event,
                                &marmot_agent_core::relay::DEFAULT_RELAYS,
                            ).await {
                                Ok(_) => println!("  KeyPackage rotated on relays."),
                                Err(e) => eprintln!("  KeyPackage publish failed: {e}"),
                            }
                        }
                        Err(e) => eprintln!("  KeyPackage signing failed: {e}"),
                    },
                    Err(e) => eprintln!("  KeyPackage creation failed: {e}"),
                }

                println!("\nRun 'receive' to fetch new messages.");
                let _ = publish; // --publish reserved for future use
            }
            GroupAction::Accept { group } => {
                let Some(ctx) = load_default_context().await else { return; };

                match ctx.accept_welcome_by_nostr_id(&group) {
                    Ok(()) => {
                        println!("Joined group: {}", group);
                        println!("  Run 'receive' to fetch messages.");
                    }
                    Err(e) => eprintln!("Failed to accept invitation: {e}"),
                }
            }
            GroupAction::Decline { group } => {
                let Some(ctx) = load_default_context().await else { return; };

                match ctx.decline_welcome_by_nostr_id(&group) {
                    Ok(()) => println!("Invitation declined: {}", group),
                    Err(e) => eprintln!("Failed to decline invitation: {e}"),
                }
            }
            GroupAction::Rename { group, name, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.rename_group(&g.mls_group_id, &name) {
                    Ok(result) => {
                        println!("Group renamed to '{}'.", name);
                        if publish {
                            let ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                            match marmot_agent_core::relay::publish_event(ev, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                Ok(r) => {
                                    let ok = r.iter().filter(|(_, ok)| *ok).count();
                                    println!("  Published: {}/{} relays OK", ok, r.len());
                                }
                                Err(e) => eprintln!("  Publish failed: {e}"),
                            }
                        } else {
                            println!("  NOTE: Use --publish to send the commit event to relays.");
                        }
                    }
                    Err(e) => eprintln!("Rename failed: {e}"),
                }
            }
            GroupAction::RemoveMembers { group, members, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                if members.is_empty() {
                    eprintln!("No members specified. Use --member <npub> to specify members.");
                    return;
                }

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                let mut pubkeys = vec![];
                for m in &members {
                    match PublicKey::parse(m) {
                        Ok(pk) => pubkeys.push(pk),
                        Err(e) => { eprintln!("Invalid npub '{}': {e}", m); return; }
                    }
                }

                match ctx.remove_group_members(&g.mls_group_id, &pubkeys) {
                    Ok(result) => {
                        println!("Removed {} member(s) from group.", pubkeys.len());
                        if publish {
                            let ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                            match marmot_agent_core::relay::publish_event(ev, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                Ok(r) => {
                                    let ok = r.iter().filter(|(_, ok)| *ok).count();
                                    println!("  Published: {}/{} relays OK", ok, r.len());
                                }
                                Err(e) => eprintln!("  Publish failed: {e}"),
                            }
                        } else {
                            println!("  NOTE: Use --publish to send the commit event to relays.");
                        }
                    }
                    Err(e) => eprintln!("Remove members failed: {e}"),
                }
            }
            GroupAction::Promote { group, member, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                let member_pk = match PublicKey::parse(&member) {
                    Ok(pk) => pk,
                    Err(e) => { eprintln!("Invalid npub '{}': {e}", member); return; }
                };

                match ctx.promote_member(&g.mls_group_id, member_pk) {
                    Ok(result) => {
                        println!("Member promoted to admin.");
                        if publish {
                            let ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                            match marmot_agent_core::relay::publish_event(ev, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                Ok(r) => {
                                    let ok = r.iter().filter(|(_, ok)| *ok).count();
                                    println!("  Published: {}/{} relays OK", ok, r.len());
                                }
                                Err(e) => eprintln!("  Publish failed: {e}"),
                            }
                        } else {
                            println!("  NOTE: Use --publish to send the commit event to relays.");
                        }
                    }
                    Err(e) => eprintln!("Promote failed: {e}"),
                }
            }
            GroupAction::Demote { group, member, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                let member_pk = match PublicKey::parse(&member) {
                    Ok(pk) => pk,
                    Err(e) => { eprintln!("Invalid npub '{}': {e}", member); return; }
                };

                match ctx.demote_member(&g.mls_group_id, &member_pk) {
                    Ok(result) => {
                        println!("Admin demoted to member.");
                        if publish {
                            let ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                            match marmot_agent_core::relay::publish_event(ev, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                Ok(r) => {
                                    let ok = r.iter().filter(|(_, ok)| *ok).count();
                                    println!("  Published: {}/{} relays OK", ok, r.len());
                                }
                                Err(e) => eprintln!("  Publish failed: {e}"),
                            }
                        } else {
                            println!("  NOTE: Use --publish to send the commit event to relays.");
                        }
                    }
                    Err(e) => eprintln!("Demote failed: {e}"),
                }
            }
            GroupAction::SelfDemote { group, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.self_demote(&g.mls_group_id) {
                    Ok(result) => {
                        println!("Removed yourself from admin list.");
                        if publish {
                            let ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                            match marmot_agent_core::relay::publish_event(ev, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                Ok(r) => {
                                    let ok = r.iter().filter(|(_, ok)| *ok).count();
                                    println!("  Published: {}/{} relays OK", ok, r.len());
                                }
                                Err(e) => eprintln!("  Publish failed: {e}"),
                            }
                        } else {
                            println!("  NOTE: Use --publish to send the commit event to relays.");
                        }
                    }
                    Err(e) => eprintln!("Self-demote failed: {e}"),
                }
            }
            GroupAction::Leave { group, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.leave_group(&g.mls_group_id) {
                    Ok(result) => {
                        println!("Left group '{}'.", g.name);
                        if publish {
                            let ev = marmot_agent_core::context::AgentContext::evolution_event(&result);
                            match marmot_agent_core::relay::publish_event(ev, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                Ok(r) => {
                                    let ok = r.iter().filter(|(_, ok)| *ok).count();
                                    println!("  Published: {}/{} relays OK", ok, r.len());
                                }
                                Err(e) => eprintln!("  Publish failed: {e}"),
                            }
                        } else {
                            println!("  NOTE: Use --publish to send the leave event to relays.");
                        }
                    }
                    Err(e) => {
                        eprintln!("Leave failed: {e}");
                        if e.to_string().contains("admin") || e.to_string().contains("demote") {
                            eprintln!("  Hint: Run 'groups self-demote --group {}' first, then try leaving again.", group);
                        }
                    }
                }
            }
        },
        Commands::Dm { action } => match action {
            DmAction::Create { recipient, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let recipient_pk = match PublicKey::parse(&recipient) {
                    Ok(pk) => pk,
                    Err(e) => {
                        eprintln!("Invalid npub '{}': {}", recipient, e);
                        return;
                    }
                };

                // Reuse an existing DM group if one already exists with this recipient.
                if let Ok(Some(existing)) = ctx.find_dm_with_peer(&recipient_pk) {
                    println!("DM with this recipient already exists — reusing it.");
                    println!("  nostr-id: {}", hex::encode(existing.nostr_group_id));
                    return;
                }

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
                let result = match ctx.create_dm("", relays, &kp_event) {
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

                    if let Some(rumors) = result.welcome_rumors {
                        let inbox = marmot_agent_core::relay::fetch_inbox_relays(
                            recipient_pk,
                            &marmot_agent_core::relay::DEFAULT_RELAYS,
                        ).await;
                        let welcome_relay_strings: Vec<String> = if inbox.is_empty() {
                            println!("    (no kind:10050 inbox relays found — falling back to default relays)");
                            marmot_agent_core::relay::DEFAULT_RELAYS.iter().map(|s| s.to_string()).collect()
                        } else {
                            println!("    recipient inbox relays: {:?}", inbox);
                            inbox
                        };

                        for rumor in rumors {
                            match ctx.gift_wrap_welcome(rumor, &recipient_pk).await {
                                Ok(gift_wrap) => {
                                    match marmot_agent_core::relay::publish_gift_wrap(
                                        &gift_wrap,
                                        &welcome_relay_strings,
                                        &marmot_agent_core::relay::DEFAULT_RELAYS,
                                        &ctx.identity.keys,
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
                                let display = if g.name.is_empty() {
                                    let peer = ctx.get_members_for_group(&g.mls_group_id).ok()
                                        .and_then(|members| {
                                            members.into_iter()
                                                .find(|pk| pk != &ctx.identity.keys.public_key())
                                                .map(|pk| marmot_agent_core::context::AgentContext::member_npub(&pk))
                                        });
                                    match peer {
                                        Some(p) => format!("<DM with {}>", p),
                                        None => "<Direct Message>".to_string(),
                                    }
                                } else {
                                    g.name.clone()
                                };
                                println!("  '{}' (nostr-id: {})", display, hex::encode(g.nostr_group_id));
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to list conversations: {}", e),
                }
            }
            DmAction::Send { group, message, reply_to, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let reply_to_id = match parse_optional_event_id(reply_to.as_deref()) {
                    Ok(id) => id,
                    Err(e) => { eprintln!("{e}"); return; }
                };

                match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => {
                        match ctx.create_message(&g.mls_group_id, &message, reply_to_id) {
                            Ok(event) => {
                                println!("Encrypted message created!");
                                println!("  Event ID: {}", event.id);
                                println!("  Kind: {}", event.kind);

                                if publish {
                                    let group_relays = ctx.get_group_relays(&g.mls_group_id).ok();
                                    publish_message_event(&event, group_relays).await;
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
            DmAction::Messages { group, limit, before, after } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_group = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => {
                        eprintln!("Conversation '{}' not found. Use 'dm list' to see available conversations.", group);
                        return;
                    }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.get_messages_for_group(&target_group.mls_group_id, if before.is_some() || after.is_some() { 10_000 } else { limit }) {
                    Ok(mut messages) => {
                        if let Some(ts) = before {
                            messages.retain(|m| m.created_at.as_secs() < ts);
                        }
                        if let Some(ts) = after {
                            messages.retain(|m| m.created_at.as_secs() > ts);
                        }
                        let messages: Vec<_> = messages.into_iter().take(limit).collect();
                        let display_name = if target_group.name.is_empty() { "<Direct Message>" } else { &target_group.name };
                        if messages.is_empty() {
                            println!("No messages in '{}'. Run 'receive' to fetch from relays.", display_name);
                        } else {
                            println!("Messages in '{}' (newest first):", display_name);
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
        Commands::Messages { action } => match action {
            MessageAction::React { group, event_id, emoji, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_id = match EventId::parse(&event_id) {
                    Ok(id) => id,
                    Err(e) => { eprintln!("Invalid event ID '{}': {e}", event_id); return; }
                };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.create_reaction(&g.mls_group_id, target_id, &emoji) {
                    Ok(event) => {
                        println!("Reaction '{}' created!", emoji);
                        println!("  Event ID: {}", event.id);
                        if publish {
                            let group_relays = ctx.get_group_relays(&g.mls_group_id).ok();
                            publish_message_event(&event, group_relays).await;
                        } else {
                            println!("  NOTE: Use --publish to send to relays.");
                        }
                    }
                    Err(e) => eprintln!("Reaction creation failed: {e}"),
                }
            }
            MessageAction::Delete { group, event_id, publish } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_id = match EventId::parse(&event_id) {
                    Ok(id) => id,
                    Err(e) => { eprintln!("Invalid event ID '{}': {e}", event_id); return; }
                };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                match ctx.create_deletion(&g.mls_group_id, target_id) {
                    Ok(event) => {
                        println!("Deletion request created.");
                        println!("  Event ID: {}", event.id);
                        if publish {
                            let group_relays = ctx.get_group_relays(&g.mls_group_id).ok();
                            publish_message_event(&event, group_relays).await;
                        } else {
                            println!("  NOTE: Use --publish to send to relays.");
                        }
                    }
                    Err(e) => eprintln!("Deletion creation failed: {e}"),
                }
            }
            MessageAction::Search { group, query, limit } => {
                let Some(ctx) = load_default_context().await else { return; };

                let g = match ctx.find_group_by_nostr_id(&group) {
                    Ok(Some(g)) => g,
                    Ok(None) => { eprintln!("Group '{}' not found.", group); return; }
                    Err(e) => { eprintln!("error: {e}"); return; }
                };

                let q = query.to_lowercase();
                match ctx.get_messages_for_group(&g.mls_group_id, 10_000) {
                    Ok(all_msgs) => {
                        let hits: Vec<_> = all_msgs.iter()
                            .filter(|m| m.content.to_lowercase().contains(&q))
                            .take(limit)
                            .collect();
                        let group_name = if g.name.is_empty() { "<Direct Message>" } else { &g.name };
                        if hits.is_empty() {
                            println!("No messages matching '{}' in '{}'.", query, group_name);
                        } else {
                            println!("Messages matching '{}' in '{}' ({} found):", query, group_name, hits.len());
                            for msg in hits {
                                let sender = marmot_agent_core::context::AgentContext::member_npub(&msg.pubkey);
                                println!("  [{}] {}: {}", msg.created_at, &sender[..16], msg.content);
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to get messages: {e}"),
                }
            }
            MessageAction::SearchAll { query, limit } => {
                let Some(ctx) = load_default_context().await else { return; };

                let q = query.to_lowercase();
                let groups = match ctx.list_groups() {
                    Ok(g) => g,
                    Err(e) => { eprintln!("Failed to list groups: {e}"); return; }
                };

                let mut total_hits = 0usize;
                for g in &groups {
                    let group_name = if g.name.is_empty() {
                        let peer = ctx.get_members_for_group(&g.mls_group_id).ok()
                            .and_then(|members| {
                                members.into_iter()
                                    .find(|pk| pk != &ctx.identity.keys.public_key())
                                    .map(|pk| marmot_agent_core::context::AgentContext::member_npub(&pk))
                            });
                        match peer {
                            Some(p) => format!("<DM with {}>", p),
                            None => "<Direct Message>".to_string(),
                        }
                    } else {
                        g.name.clone()
                    };

                    if let Ok(all_msgs) = ctx.get_messages_for_group(&g.mls_group_id, 10_000) {
                        let hits: Vec<_> = all_msgs.iter()
                            .filter(|m| m.content.to_lowercase().contains(&q))
                            .take(limit)
                            .collect();
                        if !hits.is_empty() {
                            println!("\n  '{}' ({} match(es)):", group_name, hits.len());
                            for msg in hits {
                                let sender = marmot_agent_core::context::AgentContext::member_npub(&msg.pubkey);
                                println!("    [{}] {}: {}", msg.created_at, &sender[..16], msg.content);
                            }
                            total_hits += 1;
                        }
                    }
                }
                if total_hits == 0 {
                    println!("No messages matching '{}' found in any group.", query);
                }
            }
        },
        Commands::Profile { action } => match action {
            ProfileAction::Show => {
                let Some(ctx) = load_default_context().await else { return; };

                let filter = nostr::Filter::new()
                    .kind(nostr::Kind::Metadata)
                    .author(ctx.identity.keys.public_key())
                    .limit(1);
                println!("Fetching profile from relays...");
                match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(events) => {
                        if let Some(ev) = events.into_iter().next() {
                            println!("Profile:");
                            println!("  npub: {}", ctx.npub());
                            // Parse the JSON content
                            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&ev.content) {
                                let fields = [
                                    ("name", "name"),
                                    ("display_name", "display_name"),
                                    ("about", "about"),
                                    ("picture", "picture"),
                                    ("nip05", "nip05"),
                                    ("lud16", "lud16"),
                                ];
                                for (key, label) in &fields {
                                    if let Some(val) = meta.get(key).and_then(|v| v.as_str()) {
                                        if !val.is_empty() {
                                            println!("  {}: {}", label, val);
                                        }
                                    }
                                }
                            } else {
                                println!("  (raw content): {}", ev.content);
                            }
                            println!("  updated: {}", ev.created_at);
                        } else {
                            println!("No profile found for {}.", ctx.npub());
                            println!("  Set one with: marmot-cli profile update --name <name>");
                        }
                    }
                    Err(e) => eprintln!("Failed to fetch profile: {e}"),
                }
            }
            ProfileAction::Update { name, display_name, about, picture, nip05, lud16 } => {
                let Some(ctx) = load_default_context().await else { return; };

                // Fetch existing profile to merge fields
                let existing = {
                    let filter = nostr::Filter::new()
                        .kind(nostr::Kind::Metadata)
                        .author(ctx.identity.keys.public_key())
                        .limit(1);
                    marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS)
                        .await
                        .ok()
                        .and_then(|evs| evs.into_iter().next())
                        .and_then(|ev| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&ev.content).ok())
                        .unwrap_or_default()
                };

                let mut meta = existing;
                if let Some(v) = name { meta.insert("name".into(), serde_json::Value::String(v)); }
                if let Some(v) = display_name { meta.insert("display_name".into(), serde_json::Value::String(v)); }
                if let Some(v) = about { meta.insert("about".into(), serde_json::Value::String(v)); }
                if let Some(v) = picture { meta.insert("picture".into(), serde_json::Value::String(v)); }
                if let Some(v) = nip05 { meta.insert("nip05".into(), serde_json::Value::String(v)); }
                if let Some(v) = lud16 { meta.insert("lud16".into(), serde_json::Value::String(v)); }

                let content = serde_json::to_string(&meta).unwrap_or_default();
                let event = match nostr::EventBuilder::new(nostr::Kind::Metadata, &content)
                    .sign_with_keys(&ctx.identity.keys)
                {
                    Ok(e) => e,
                    Err(e) => { eprintln!("Failed to build profile event: {e}"); return; }
                };

                match marmot_agent_core::relay::publish_event(&event, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(results) => {
                        let ok = results.iter().filter(|(_, ok)| *ok).count();
                        println!("Profile updated and published: {}/{} relays OK", ok, results.len());
                        println!("  Event ID: {}", event.id);
                    }
                    Err(e) => eprintln!("Publish failed: {e}"),
                }
            }
        },
        Commands::Users { action } => match action {
            UsersAction::Show { npub } => {
                let target_pk = match PublicKey::parse(&npub) {
                    Ok(pk) => pk,
                    Err(e) => { eprintln!("Invalid npub '{}': {e}", npub); return; }
                };

                let filter = nostr::Filter::new()
                    .kind(nostr::Kind::Metadata)
                    .author(target_pk)
                    .limit(1);
                println!("Fetching profile for {}...", npub);
                match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(events) => {
                        if let Some(ev) = events.into_iter().next() {
                            println!("Profile:");
                            println!("  npub: {}", marmot_agent_core::context::AgentContext::member_npub(&target_pk));
                            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&ev.content) {
                                for field in &["name", "display_name", "about", "picture", "nip05", "lud16", "website"] {
                                    if let Some(val) = meta.get(field).and_then(|v| v.as_str()) {
                                        if !val.is_empty() {
                                            println!("  {}: {}", field, val);
                                        }
                                    }
                                }
                            } else {
                                println!("  (raw): {}", ev.content);
                            }
                        } else {
                            println!("No profile found for {}.", npub);
                        }
                    }
                    Err(e) => eprintln!("Failed to fetch profile: {e}"),
                }
            }
        },
        Commands::Follows { action } => match action {
            FollowsAction::List => {
                let Some(ctx) = load_default_context().await else { return; };

                let filter = nostr::Filter::new()
                    .kind(nostr::Kind::ContactList)
                    .author(ctx.identity.keys.public_key())
                    .limit(1);
                println!("Fetching follow list from relays...");
                match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(events) => {
                        if let Some(ev) = events.into_iter().next() {
                            let follows: Vec<PublicKey> = ev.tags.iter()
                                .filter(|t| t.kind() == nostr::TagKind::p())
                                .filter_map(|t| t.content().and_then(|s| PublicKey::parse(s).ok()))
                                .collect();
                            if follows.is_empty() {
                                println!("Follow list is empty.");
                            } else {
                                println!("Following ({}):", follows.len());
                                for pk in follows {
                                    println!("  {}", marmot_agent_core::context::AgentContext::member_npub(&pk));
                                }
                            }
                        } else {
                            println!("No follow list found. Use 'follows add <npub>' to start following.");
                        }
                    }
                    Err(e) => eprintln!("Failed to fetch follow list: {e}"),
                }
            }
            FollowsAction::Add { npub } => {
                let Some(ctx) = load_default_context().await else { return; };

                let new_pk = match PublicKey::parse(&npub) {
                    Ok(pk) => pk,
                    Err(e) => { eprintln!("Invalid npub '{}': {e}", npub); return; }
                };

                let filter = nostr::Filter::new()
                    .kind(nostr::Kind::ContactList)
                    .author(ctx.identity.keys.public_key())
                    .limit(1);
                let existing_tags = marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS)
                    .await
                    .ok()
                    .and_then(|evs| evs.into_iter().next())
                    .map(|ev| ev.tags.into_iter().collect::<Vec<_>>())
                    .unwrap_or_default();

                let already_following = existing_tags.iter()
                    .filter(|t| t.kind() == nostr::TagKind::p())
                    .filter_map(|t| t.content().and_then(|s| PublicKey::parse(s).ok()))
                    .any(|pk| pk == new_pk);

                if already_following {
                    println!("Already following {}.", npub);
                    return;
                }

                let mut tags = existing_tags;
                tags.push(nostr::Tag::public_key(new_pk));

                let event = match nostr::EventBuilder::new(nostr::Kind::ContactList, "")
                    .tags(tags)
                    .sign_with_keys(&ctx.identity.keys)
                {
                    Ok(e) => e,
                    Err(e) => { eprintln!("Failed to build contact list: {e}"); return; }
                };

                match marmot_agent_core::relay::publish_event(&event, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(results) => {
                        let ok = results.iter().filter(|(_, ok)| *ok).count();
                        println!("Now following {}. Published: {}/{} relays OK", npub, ok, results.len());
                    }
                    Err(e) => eprintln!("Publish failed: {e}"),
                }
            }
            FollowsAction::Remove { npub } => {
                let Some(ctx) = load_default_context().await else { return; };

                let target_pk = match PublicKey::parse(&npub) {
                    Ok(pk) => pk,
                    Err(e) => { eprintln!("Invalid npub '{}': {e}", npub); return; }
                };

                let filter = nostr::Filter::new()
                    .kind(nostr::Kind::ContactList)
                    .author(ctx.identity.keys.public_key())
                    .limit(1);
                let existing_tags = marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS)
                    .await
                    .ok()
                    .and_then(|evs| evs.into_iter().next())
                    .map(|ev| ev.tags.into_iter().collect::<Vec<_>>())
                    .unwrap_or_default();

                let before_len = existing_tags.len();
                let tags: Vec<nostr::Tag> = existing_tags.into_iter()
                    .filter(|t| {
                        if t.kind() == nostr::TagKind::p() {
                            t.content().and_then(|s| PublicKey::parse(s).ok())
                                .map(|pk| pk != target_pk)
                                .unwrap_or(true)
                        } else {
                            true
                        }
                    })
                    .collect();

                if tags.len() == before_len {
                    println!("Not currently following {}.", npub);
                    return;
                }

                let event = match nostr::EventBuilder::new(nostr::Kind::ContactList, "")
                    .tags(tags)
                    .sign_with_keys(&ctx.identity.keys)
                {
                    Ok(e) => e,
                    Err(e) => { eprintln!("Failed to build contact list: {e}"); return; }
                };

                match marmot_agent_core::relay::publish_event(&event, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                    Ok(results) => {
                        let ok = results.iter().filter(|(_, ok)| *ok).count();
                        println!("Unfollowed {}. Published: {}/{} relays OK", npub, ok, results.len());
                    }
                    Err(e) => eprintln!("Publish failed: {e}"),
                }
            }
        },
        Commands::Chats { action } => match action {
            ChatsAction::List { limit } => {
                let Some(ctx) = load_default_context().await else { return; };

                let groups = match ctx.list_groups() {
                    Ok(g) => g,
                    Err(e) => { eprintln!("Failed to list groups: {e}"); return; }
                };

                if groups.is_empty() {
                    println!("No conversations found.");
                    return;
                }

                struct ChatPreview {
                    display_name: String,
                    nostr_id: String,
                    last_ts: u64,
                    last_sender: String,
                    last_content: String,
                }

                let mut previews: Vec<ChatPreview> = vec![];
                for g in groups.iter().take(limit) {
                    let display_name = if g.name.is_empty() {
                        let peer = ctx.get_members_for_group(&g.mls_group_id).ok()
                            .and_then(|members| {
                                members.into_iter()
                                    .find(|pk| pk != &ctx.identity.keys.public_key())
                                    .map(|pk| marmot_agent_core::context::AgentContext::member_npub(&pk))
                            });
                        match peer {
                            Some(p) => {
                                let truncated = &p[..std::cmp::min(p.len(), 20)];
                                format!("<DM with {}>", truncated)
                            }
                            None => "<Direct Message>".to_string(),
                        }
                    } else {
                        g.name.clone()
                    };

                    let (last_ts, last_sender, last_content) =
                        if let Ok(msgs) = ctx.get_messages_for_group(&g.mls_group_id, 1) {
                            if let Some(msg) = msgs.first() {
                                let sender = marmot_agent_core::context::AgentContext::member_npub(&msg.pubkey);
                                let preview: String = msg.content.chars().take(60).collect();
                                let preview = if msg.content.len() > 60 { format!("{}…", preview) } else { preview };
                                let ts = msg.created_at.as_secs();
                                let sender_short = sender[..std::cmp::min(sender.len(), 16)].to_string();
                                (ts, sender_short, preview)
                            } else {
                                (0u64, String::new(), "(no messages)".to_string())
                            }
                        } else {
                            (0u64, String::new(), "(no messages)".to_string())
                        };

                    previews.push(ChatPreview {
                        display_name,
                        nostr_id: hex::encode(g.nostr_group_id),
                        last_ts,
                        last_sender,
                        last_content,
                    });
                }

                // Sort by most recent message first
                previews.sort_by(|a, b| b.last_ts.cmp(&a.last_ts));

                println!("Conversations ({}):", previews.len());
                for p in &previews {
                    if p.last_ts > 0 {
                        println!("  '{}' ({})", p.display_name, p.nostr_id);
                        println!("    [{}] {}: {}", p.last_ts, p.last_sender, p.last_content);
                    } else {
                        println!("  '{}' ({}) — no messages", p.display_name, p.nostr_id);
                    }
                }
            }
        },
        Commands::Receive { limit, offline } => {
            let Some(ctx) = load_default_context().await else { return; };

            let groups = match ctx.list_groups() {
                Ok(g) => g,
                Err(e) => { eprintln!("Failed to list groups: {e}"); return; }
            };

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

            let gift_wrap_events = if offline {
                vec![]
            } else {
                println!("Checking for group invitations (gift wraps)...");
                let our_pubkey = ctx.identity.keys.public_key();
                let inbox = marmot_agent_core::relay::fetch_inbox_relays(
                    our_pubkey,
                    &marmot_agent_core::relay::DEFAULT_RELAYS,
                ).await;
                let mut fetch_relays: Vec<String> = marmot_agent_core::relay::DEFAULT_RELAYS
                    .iter().map(|s| s.to_string()).collect();
                for r in &inbox {
                    if !fetch_relays.contains(r) {
                        fetch_relays.push(r.clone());
                    }
                }
                let fetch_relay_refs: Vec<&str> = fetch_relays.iter().map(|s| s.as_str()).collect();
                match marmot_agent_core::relay::fetch_gift_wrap_events(
                    our_pubkey,
                    &fetch_relay_refs,
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
        Commands::Debug { pubkey } => {
            let Some(ctx) = load_default_context().await else { return; };

            let target_pk = match PublicKey::parse(&pubkey) {
                Ok(pk) => pk,
                Err(e) => { eprintln!("Invalid pubkey '{}': {e}", pubkey); return; }
            };
            let target_hex = target_pk.to_hex();
            let our_pk = ctx.identity.keys.public_key();

            println!("=== Diagnosing relay events ===");
            println!("  Target pubkey : {}", target_hex);
            println!("  Our pubkey    : {}", our_pk.to_hex());
            println!();

            println!("--- Events authored by target (kinds 445/10449/4459) ---");
            let filter = nostr::Filter::new()
                .author(target_pk)
                .kinds(vec![nostr::Kind::Custom(445), nostr::Kind::Custom(10449), nostr::Kind::Custom(4459)])
                .limit(50);
            match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                Ok(events) => {
                    if events.is_empty() {
                        println!("  (none found)");
                    }
                    for ev in &events {
                        let h_tags: Vec<String> = ev.tags.iter()
                            .filter(|t| t.kind() == nostr::TagKind::SingleLetter(nostr::SingleLetterTag::lowercase(nostr::Alphabet::H)))
                            .filter_map(|t| t.content().map(|s| s.to_string()))
                            .collect();
                        println!("  kind={} id={} created={} h-tags={:?}",
                            ev.kind, &ev.id.to_hex()[..12], ev.created_at, h_tags);
                    }
                }
                Err(e) => eprintln!("  fetch failed: {e}"),
            }
            println!();

            println!("--- Gift wraps addressed to us (kind 1059) ---");
            let filter = nostr::Filter::new()
                .kind(nostr::Kind::GiftWrap)
                .pubkey(our_pk)
                .limit(50);
            match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                Ok(events) => {
                    if events.is_empty() {
                        println!("  (none found)");
                    }
                    for ev in &events {
                        print!("  id={} created={} outer_pubkey={} → ",
                            &ev.id.to_hex()[..12], ev.created_at, &ev.pubkey.to_hex()[..12]);
                        match ctx.unwrap_and_process_welcome(&ev).await {
                            Ok(Some(w)) => println!("kind=444 welcome, nostr_group={}", hex::encode(w.nostr_group_id)),
                            Ok(None) => {
                                match nostr::nips::nip59::extract_rumor(&ctx.identity.keys, ev).await {
                                    Ok(unwrapped) => println!("kind={} (not a welcome)", unwrapped.rumor.kind),
                                    Err(e) => println!("unwrap failed: {e}"),
                                }
                            }
                            Err(e) => println!("process_welcome error: {e}"),
                        }
                    }
                }
                Err(e) => eprintln!("  fetch failed: {e}"),
            }
            println!();

            println!("--- Gift wraps authored by target (kind 1059) ---");
            let filter = nostr::Filter::new()
                .kind(nostr::Kind::GiftWrap)
                .author(target_pk)
                .limit(50);
            match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                Ok(events) => {
                    if events.is_empty() {
                        println!("  (none — gift wraps use random outer keys, so author filter won't work)");
                    } else {
                        for ev in &events {
                            println!("  id={} created={}", &ev.id.to_hex()[..12], ev.created_at);
                        }
                    }
                }
                Err(e) => eprintln!("  fetch failed: {e}"),
            }
            println!();

            println!("--- Events on relay for shared groups ---");
            match ctx.list_groups() {
                Ok(groups) => {
                    let target_short = &target_hex[..16];
                    let relevant: Vec<_> = groups.iter()
                        .filter(|g| g.name.contains(target_short) || g.name.contains(&target_hex))
                        .collect();
                    if relevant.is_empty() {
                        for g in &groups {
                            println!("  {} ({})", hex::encode(g.nostr_group_id), g.name);
                        }
                    } else {
                        for g in &relevant {
                            let h = hex::encode(g.nostr_group_id);
                            println!("  Group '{}' h={}", g.name, h);
                            let filter = nostr::Filter::new()
                                .kinds(vec![
                                    nostr::Kind::Custom(445),
                                    nostr::Kind::Custom(10449),
                                    nostr::Kind::Custom(4459),
                                ])
                                .custom_tags(
                                    nostr::SingleLetterTag::lowercase(nostr::Alphabet::H),
                                    [h.clone()],
                                )
                                .limit(20);
                            match marmot_agent_core::relay::fetch_raw(filter, &marmot_agent_core::relay::DEFAULT_RELAYS).await {
                                Ok(evs) => {
                                    for ev in &evs {
                                        println!("    kind={} id={} author={} created={}",
                                            ev.kind, &ev.id.to_hex()[..16],
                                            &ev.pubkey.to_hex()[..12], ev.created_at);
                                    }
                                    if evs.is_empty() { println!("    (no events found)"); }
                                }
                                Err(e) => eprintln!("    fetch failed: {e}"),
                            }
                        }
                    }
                }
                Err(e) => eprintln!("  {e}"),
            }
        }
    }
}

fn parse_optional_event_id(s: Option<&str>) -> anyhow::Result<Option<EventId>> {
    match s {
        None => Ok(None),
        Some(id_str) => EventId::parse(id_str)
            .map(Some)
            .map_err(|e| anyhow::anyhow!("Invalid event ID '{}': {e}", id_str)),
    }
}
