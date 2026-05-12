# marmot-cli — Feature Parity TODO

> Compared against [whitenoise-rs](https://github.com/marmot-protocol/whitenoise-rs), the Marmot team's reference CLI implementation.
> This is a **feature** list, not an architecture list — marmot-cli's single-binary model is intentional.
> Items are grouped by area and ranked roughly by priority for agent use cases.

---

## ✅ Already Implemented

- Identity: create, list, show, delete, set-default
- KeyPackage: publish (kind 30443), show
- DM: create (deduplicates — reuses existing group if recipient already has one), list (shows peer npub), send (with `--reply-to`), messages
- Groups: create (with `--description`), invite, list, members, messages, pending, join (bulk-accept)
- Groups: show, send (with `--reply-to`), leave, decline, accept, rename, remove-members, promote, demote, self-demote
- Messages: react (kind 7), delete (kind 5)
- Receive: fetches kind 445/10449/4459 + kind 1059 gift-wraps; decrypts and stores all
- Relay: list default relays
- Daemon: TCP JSON-RPC skeleton (`ping` live; other methods are stubs)
- Interop confirmed with White Noise (iOS): DM grouping (empty name), inbox relay delivery (NIP-42), inner rumor kind 9

---

## 🔴 High Priority — Core Messaging Completeness

### Group lifecycle
- [x] `groups leave <group_id>` — publish a SelfRemove proposal + leave locally
- [x] `groups decline <group_id>` — decline a pending invitation (mark locally; no relay action needed)
- [x] `groups accept <group_id>` — accept a single named invitation (currently `groups join` accepts all pending)
- [x] `groups rename <group_id> <name>` — update NostrGroupDataExtension name via GroupContextExtensions commit
- [x] `groups remove-members <group_id> <npubs...>` — admin removes one or more members
- [x] `groups promote <group_id> <pubkey>` — add pubkey to admin list in extension
- [x] `groups demote <group_id> <pubkey>` — remove pubkey from admin list in extension
- [x] `groups self-demote <group_id>` — remove self from admin list (required before leaving if admin)
- [x] `groups show <group_id>` — show full group metadata: name, admins, relays, member count

### Message operations
- [x] `--reply-to <message_id>` flag on `dm send` and `groups send` — include reply thread tag
- [x] `messages delete <group_id> <event_id>` — publish kind 5 deletion event
- [x] `messages react <group_id> <message_id> [--emoji <char>]` — send kind 7 reaction (inner rumor)
- [ ] `messages unreact <group_id> <message_id>` — remove own reaction
- [ ] `messages retry <group_id> <event_id>` — re-send a failed message as a new event

### Groups create — missing options
- [x] `--description <text>` flag for `groups create`
- [ ] `--members <npubs...>` flag for `groups create` — create group with initial members in one step (fetch their KeyPackages, add all, send welcome to each)

---

## 🟡 Medium Priority — Relay & Key Package Management

### Relay management (currently only `relay list`)
- [ ] `relays add <url> --type <inbox|key_package|nip65>` — add a relay URL to the specified list and publish updated kind 10050 / 10051 / 10002
- [ ] `relays remove <url> --type <inbox|key_package|nip65>` — remove and republish
- [ ] `relays list` — show all three relay categories (inbox, key_package, nip65) with connection status
- [ ] Publish inbox relay list (kind 10050) on first run / identity creation — required so others can deliver gift-wraps
- [ ] Publish key package relay list (kind 10051) on keypackage publish — currently not published

### Key package management
- [ ] `keypackage list` — list all our key packages currently on relays (by event ID + timestamp)
- [ ] `keypackage delete <event_id>` — delete a specific key package event from relays
- [ ] `keypackage delete-all --confirm` — delete all our key packages from relays
- [ ] `keypackage check <npub>` — check if a given user has a valid key package on relays
- [ ] Background key package maintenance (daemon task): publish fresh packages, remove consumed/expired (>30 days old)

### Profile
- [ ] `profile show` — fetch and display our own Nostr metadata (kind 0)
- [ ] `profile update [--name] [--display-name] [--about] [--picture <url>] [--nip05] [--lud16]` — update and publish kind 0 profile metadata

---

## 🟡 Medium Priority — Message History & Search

- [ ] Cursor-based pagination for messages: `--before <timestamp>`, `--after <timestamp>`, `--before-id <event_id>` flags on `dm messages` / `groups messages`
- [ ] `messages search <group_id> <query>` — substring search within a group's stored messages
- [ ] `messages search-all <query>` — search across all groups

---

## 🟠 Medium Priority — Daemon Background Tasks

Once the daemon is fully wired (Phase 6), add scheduled maintenance loops:

- [ ] Key package refresh — every 10 min: publish fresh packages, clean up consumed ones
- [ ] Subscription health check — every 15 min: verify relay subscriptions are alive, rebuild if drifted
- [ ] Relay list repair — every 30 min: detect and republish inbox/key-package relay lists that failed to publish at login
- [ ] Receive loop — configurable poll interval (e.g., every 30s): fetch and process new messages automatically without the user calling `receive`

---

## 🟢 Lower Priority — Social & Discovery

### User discovery
- [ ] `users show <npub>` — fetch and display a user's Nostr profile from relays
- [ ] `users search <query>` — search users by name across followed accounts and relays

### Follows (NIP-02)
- [ ] `follows list` — list followed accounts (kind 3)
- [ ] `follows add <npub>` — follow a user
- [ ] `follows remove <npub>` — unfollow a user

### Blocking (NIP-51 mute list)
- [ ] `block add <npub>` — add to block list (private entry in kind 10000)
- [ ] `block remove <npub>` — unblock
- [ ] `block list` — list blocked users

---

## 🟢 Lower Priority — Chat Management

- [ ] `chats list` — unified view of all conversations with last-message preview and unread counts (replaces separate `dm list` / `groups list`)
- [ ] `chats archive <group_id>` / `chats unarchive <group_id>` — hide/restore a conversation from the main list
- [ ] `chats mute <group_id> <duration>` / `chats unmute <group_id>` — suppress notifications (durations: `1h`, `8h`, `1d`, `1w`, `forever`)
- [ ] Unread count tracking — track last-read event ID per group; expose unread count in `dm list` / `groups list`

---

## 🔵 Stretch — Media & Notifications

### Media attachments (MIP-04)
- [ ] `media upload <group_id> <file_path> [--message <caption>]` — encrypt and upload to Blossom server; send media message
- [ ] `media download <group_id> <file_hash>` — download and decrypt media by SHA-256 hash
- [ ] `media list <group_id>` — list media files shared in a group

### Notifications streaming
- [ ] `notifications subscribe` — stream live notification events (new messages + invitations) — useful for daemon-connected agents
- [ ] Notification payload: trigger type, group ID + name, is_dm flag, sender info, message content, timestamp

---

## Notes

- **Architecture difference**: whitenoise-rs uses a two-binary model (`wnd` daemon + `wn` client); marmot-cli uses a single binary with optional `daemon` subcommand. Feature targets don't require matching this split.
- **`--json` output flag**: whitenoise-rs outputs structured JSON for all commands. Adding `--json` to marmot-cli commands would make agent integration cleaner (no line-parsing needed). Worth doing alongside daemon work.
- **Kind 443 vs 30443**: whitenoise-rs publishes to both for backward compatibility. marmot-cli only publishes kind 30443. Consider publishing both.
- **SelfRemove proposal**: required for voluntary `groups leave`. Need to verify mdk-core supports this proposal type before implementing.
