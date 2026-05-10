# marmot-cli — Handoff Document

> Written for the next agent. Pick up from here.

## Current State (as of commit `HEAD` on main)

### What Works
1. **Identity management** — create, list, show, delete, set-default. Persisted to `~/.local/share/marmot-cli/identities/`. Secret keys in `.nsec` files.
2. **Encrypted SQLite storage** — `MdkSqliteStorage` with auto-generated AES key at `~/.local/share/marmot-cli/db.key`.
3. **KeyPackage publishing** — `keypackage publish` generates kind 30443 and publishes to 3 default relays.
4. **Group creation** — `groups create --name <X> --publish` creates MLS group; with `--publish` sends welcome events to relays.
5. **DM creation** — `dm create --recipient <npub> --publish` fetches recipient KeyPackage from relays, creates 2-member MLS group; with `--publish` sends evolution_commit + welcome events.
6. **DM sending** — `dm send --group <h-tag-hex> --message <msg> --publish` creates kind 445 encrypted message; resolves `h`-tag (nostr group ID) to internal MLS group ID. With `--publish` sends to relays.
    - **Note**: The `--group` parameter takes the **nostr group ID** (the 32-byte hex from the `h` tag in published events), NOT the raw MLS group ID.
7. **Daemon (TCP)** — `daemon --listen 127.0.0.1:9222`. Cross-platform (Unix socket removed).

### Architecture
- `crates/marmot-agent-core/` — identity, storage, relay, context (MDK wrapper)
- `crates/marmot-agent-cli/` — `clap` CLI entry point
- `crates/marmot-agent-rpc/` — JSON-RPC TCP server (currently ping + stubs)
- `crates/marmot-agent-ffi/` — empty placeholder

### What's Missing / Next Steps

#### Phase 4 — Group Lifecycle (in progress)
- [ ] **Incoming message processing** — subscribe to relay group filters (kind 445), decrypt received messages via `mdk.process_incoming_message()`, store plaintext in SQLite, list them in CLI.
- [ ] **Invite flow** — `groups invite --group <id> --member <npub> --publish`. Fetches member KeyPackage, calls `mdk.add_members()`, publishes resulting events.
- [ ] **Group join** — `groups join --event-id <id>` (recipient side). Receives welcome rumor, processes it via `mdk.process_welcome_event()`, creates local group.
- [ ] **Receive + render messages** — `groups messages --group <id>` or `dm messages --recipient <npub>`. Query SQLite storage for decrypted messages.
- [ ] **KeyPackage refresh** — re-publish when key expires or identity rotates.

#### Phase 5 — Tooling + CI
- [ ] `justfile` with `build`, `test`, `lint`, `fmt`
- [ ] GitHub Actions: `cargo test`, `cargo fmt --check`, `cargo clippy`
- [ ] `CHANGELOG.md` per crate
- [ ] `AGENTS.md` — high-level architecture doc
- [ ] `README.md` update with install + usage instructions
- [ ] Pre-commit hooks or `cargo-deny` for dependency scanning

#### Phase 6 — Integration
- [ ] Nix flake or `home-manager` module for dev install
- [ ] Hermes OpenClaw adapter — JSON-RPC client over TCP connecting to `marmot-cli daemon`

### Design Notes
- **Nostr group ID vs MLS group ID**: MDK stores both. The `h` tag in published events is the `nostr_group_id` (32-byte hex, derived from MLS state hash), while `mls_group_id` is the raw MLS opaque byte vector. The CLI `dm send` command now resolves by `nostr_group_id` via `find_group_by_nostr_id()` so users can copy the visible `h`-tag hex from relay events or `nak req` output.
- **No secrets in repo**. Identity files and `db.key` live in `~/.local/share/marmot-cli/` (not in git).
- `.gitignore` excludes `*.nsec`, `*.key`, `*.db`, `Cargo.lock`, `/target`.
- Relay publish is **opt-in** via `--publish` flag. Events are created + persisted locally by default.
- `keypackage publish` was run once manually for `test-agent` npub. Future publishes require explicit consent.

### File Map
```
crates/
  marmot-agent-core/
    src/
      lib.rs         # Error types + Config trait
      identity.rs    # Identity struct (Keys wrapper)
      storage.rs     # AgentStorage + AgentDirs (XDG via directories)
      context.rs     # AgentContext (MDK + Identity + Storage) -- DM/group ops
      relay.rs       # publish_event, publish_events, fetch_keypackage
  marmot-agent-cli/
    src/main.rs      # CLI commands: identity, relay, keypackage, daemon, groups, dm
  marmot-agent-rpc/
    src/server.rs    # JSON-RPC over TCP (serve_tcp_blocking)
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

// context.rs
pub fn create_dm(name, relays, member_kp_event) -> Result<UpdateGroupResult>;
pub fn prepare_group_update_events(&result) -> Result<Vec<(&str, Event)>>;
pub fn create_dm_message(mls_group_id, content) -> Result<Event>;
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

# List identities/groups/conversations
./target/release/marmot-cli identity list
./target/release/marmot-cli groups list
./target/release/marmot-cli dm list

# Create + publish a DM
e="./target/release/marmot-cli"
$e dm create --recipient npub1vl73xzhpyucxjt5dvam2zyfsllffc4kzwdn9rppym3ck5twpedlsamyt49 --publish

# Send + publish a message
$e dm send --group <GROUP_HEX> --message "hello" --publish
```

### Known Issues
- Dead code warning: `Request.jsonrpc` field in `marmot-agent-rpc/src/server.rs`
- Pre-existing lint artifacts: `async fn` edition warnings on some files (harmless, doesn't fail build)
- Daemon JSON-RPC methods are stubs (`identity_npub`, `list_groups`, `send_message`)
- Invite flow (`groups invite`, `groups join`) not implemented
- No incoming message processing yet
- `marmot-agent-ffi` crate is empty
