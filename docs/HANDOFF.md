# marmot-cli — Handoff Document

> Written for the next agent. Pick up from here.

## Current State (as of commit `HEAD` on main)

### Phase 4 Complete

### What Works
1. **Identity management** — create, list, show, delete, set-default. Persisted to `~/.local/share/marmot-cli/identities/`. Secret keys written atomically at 0o600.
2. **Encrypted SQLite storage** — `MdkSqliteStorage` with auto-generated AES key at `~/.local/share/marmot-cli/db.key` (also written atomically at 0o600).
3. **KeyPackage publishing** — `keypackage publish` generates kind 30443 and publishes to 3 default relays.
4. **KeyPackage show** — `keypackage show` fetches our own KeyPackage from relays and displays event ID + timestamp.
5. **Group creation** — `groups create --name <X> --publish` creates MLS group; with `--publish` sends welcome events to relays.
6. **DM creation** — `dm create --recipient <npub> --publish` fetches recipient KeyPackage from relays, creates 2-member MLS group; with `--publish` sends evolution_commit + welcome events.
7. **DM sending** — `dm send --group <h-tag-hex> --message <msg> --publish` creates kind 445 encrypted message (inner rumor is kind 9 per MIP-03), resolves `h`-tag to MLS group. With `--publish` sends to relays.
8. **Groups invite** — `groups invite --group <h-tag> --member <npub> --publish` fetches recipient KeyPackage, calls add_members, optionally publishes commit + welcome.
9. **Receive** — `receive [--limit N] [--offline]` fetches kind 445/10449/4459 events from all known group h-tags AND kind 1059 gift-wrap events (NIP-59 welcome invitations). Decrypts + stores via MDK.
10. **Message reading** — `groups messages --group <h-tag> [--limit N]` and `dm messages --group <h-tag>` show stored decrypted messages from SQLite.
11. **Group members** — `groups members --group <h-tag>` lists current MLS group members.
12. **Groups pending** — `groups pending` lists locally-stored pending welcome invitations (kind 444).
13. **Groups join** — `groups join` accepts all pending welcomes. Self-update is intentionally NOT run automatically (White Noise also disables post-welcome self-update — running it immediately causes epoch ordering issues where replies fail to decrypt at the other side).
14. **Daemon (TCP)** — `daemon --listen 127.0.0.1:9222`. JSON-RPC methods: ping (live), identity_npub / list_groups / send_message (stubs).

### Architecture
- `crates/marmot-agent-core/` — identity, storage, relay, context (MDK wrapper)
- `crates/marmot-agent-cli/` — `clap` CLI entry point
- `crates/marmot-agent-rpc/` — JSON-RPC TCP server (currently ping + stubs)
- `crates/marmot-agent-ffi/` — empty placeholder

#### Phase 5 — Tooling + CI
- [ ] `justfile` with `build`, `test`, `lint`, `fmt`
- [ ] GitHub Actions: `cargo test`, `cargo fmt --check`, `cargo clippy`
- [ ] `CHANGELOG.md` per crate
- [ ] `AGENTS.md` — high-level architecture doc for AI agents
- [ ] `README.md` update with install + usage instructions
- [ ] Pre-commit hooks or `cargo-deny` for dependency scanning

#### Phase 6 — Integration
- [ ] Nix flake or `home-manager` module for dev install
- [ ] Hermes OpenClaw adapter — JSON-RPC client over TCP connecting to `marmot-cli daemon`

### Design Notes
- **Nostr group ID vs MLS group ID**: MDK stores both. The `h` tag in published events is the `nostr_group_id` (32-byte hex, from `Group.nostr_group_id: [u8; 32]`), while `mls_group_id` is the raw MLS opaque byte vector (`GroupId`). All CLI `--group` flags take the nostr group ID hex (h-tag), resolved via `find_group_by_nostr_id()`.
- **Relay publish is opt-in** via `--publish` flag. Events are created + persisted locally by default.
- **No secrets in repo**. Identity `.nsec` files and `db.key` live in `~/.local/share/marmot-cli/` — written with `O_CREAT | mode(0o600)` atomically to avoid TOCTOU.
- `.gitignore` excludes `*.nsec`, `*.key`, `*.db`, `Cargo.lock`, `/target`.

### File Map
```
crates/
  marmot-agent-core/
    src/
      lib.rs         # Error types (Relay/Identity/Group/Storage/Io/Serialization/Any) + Config
      identity.rs    # Identity struct (nostr::Keys wrapper), atomic secret file write
      storage.rs     # AgentStorage + AgentDirs (XDG), config, db encryption key
      context.rs     # AgentContext (MDK + Identity + Storage) — all group/DM/message ops
      relay.rs       # publish_event, publish_events, fetch_keypackage, fetch_group_events, fetch_gift_wrap_events
  marmot-agent-cli/
    src/main.rs      # CLI: identity, relay, keypackage, daemon, groups, dm, receive
  marmot-agent-rpc/
    src/server.rs    # JSON-RPC over TCP (serve_tcp_blocking, one thread per client)
  marmot-agent-ffi/
    src/lib.rs       # placeholder
docs/
  PLAN.md            # 5-phase roadmap
```

### Key APIs (already implemented)

```rust
// relay.rs
pub async fn publish_event(event: &Event, relays: &[&str]) -> Result<Vec<(String, bool)>>;
pub async fn publish_events(events: &[(&str, &Event)], relays: &[&str]) -> Result<Vec<(&str, Vec<(String, bool)>)>>;
pub async fn fetch_keypackage(pubkey: PublicKey, relays: &[&str]) -> Result<Option<Event>>;
pub async fn fetch_group_events(h_tags: &[String], limit: usize, relays: &[&str]) -> Result<Vec<Event>>;
pub async fn fetch_gift_wrap_events(pubkey: PublicKey, relays: &[&str]) -> Result<Vec<Event>>;

// context.rs
pub fn create_group(name, relays) -> Result<GroupResult>;
pub fn create_dm(name, relays, member_kp_event) -> Result<UpdateGroupResult>;
pub fn invite_member_to_group(mls_group_id, kp_event) -> Result<UpdateGroupResult>;
pub fn prepare_group_update_events(&result) -> Result<Vec<(&str, Event)>>;
pub fn create_dm_message(mls_group_id, content) -> Result<Event>;
pub fn process_incoming_event(event) -> Result<MessageProcessingResult>;
pub fn get_messages_for_group(mls_group_id, limit) -> Result<Vec<message_types::Message>>;
pub fn get_members_for_group(mls_group_id) -> Result<BTreeSet<PublicKey>>;
pub fn find_group_by_nostr_id(hex) -> Result<Option<group_types::Group>>;
pub fn delete_group(mls_group_id) -> Result<()>;
pub fn list_groups() -> Result<Vec<group_types::Group>>;
pub fn list_pending_welcomes() -> Result<Vec<welcome_types::Welcome>>;
pub async fn unwrap_and_process_welcome(gift_wrap: &Event) -> Result<Option<welcome_types::Welcome>>;
pub fn accept_welcome(welcome: &welcome_types::Welcome) -> Result<()>;
pub fn groups_needing_self_update(threshold_secs: u64) -> Result<Vec<GroupId>>;
pub fn self_update_group(mls_group_id: &GroupId) -> Result<UpdateGroupResult>;
pub fn nostr_group_id_hex(group: &Group) -> String;  // static helper
pub fn member_npub(pk: &PublicKey) -> String;         // static helper
```

### Default Relays
`wss://nos.lol`, `wss://relay.primal.net`, `wss://relay.damus.io`

### Dev Test Identities (local, throwaway)
| Name | npub | Role |
|------|------|------|
| test-agent | npub1f76kdse35r8nvtrhz2rhn4khzg30qn7wffsx69h9qmua0a8kgcfsx6gvnd | default, KeyPackage live on relays |
| test-agent-2 | npub1vl73xzhpyucxjt5dvam2zyfsllffc4kzwdn9rppym3ck5twpedlsamyt49 | secondary |

### Useful Commands
```bash
# Build
cd ~/projects/marmot-cli && cargo build --release
e="./target/release/marmot-cli"

# Identity / KeyPackage
$e identity list
$e keypackage publish

# Create + publish a DM, then send a message
$e dm create --recipient npub1vl73xzhpyucxjt5dvam2zyfsllffc4kzwdn9rppym3ck5twpedlsamyt49 --publish
$e dm send --group <GROUP_HEX> --message "hello" --publish

# Fetch incoming messages + group invitations, then read
$e receive --limit 100
$e dm messages --group <GROUP_HEX>
$e groups messages --group <GROUP_HEX>

# Invite a member to a group
$e groups invite --group <GROUP_HEX> --member <NPUB> --publish

# See who's in a group
$e groups members --group <GROUP_HEX>

# Check + accept group invitations (recipient side)
$e groups pending
$e groups join --publish

# Show our published KeyPackage
$e keypackage show
```

### Known Issues
- Dead code warning: `Request.jsonrpc` field in `marmot-agent-rpc/src/server.rs` (harmless)
- Daemon JSON-RPC methods are stubs (`identity_npub`, `list_groups`, `send_message`)
- `marmot-agent-ffi` crate is empty
- Relay publish results are per-broadcast, not per-relay (pool sends to all at once; per-relay status is inferred)
