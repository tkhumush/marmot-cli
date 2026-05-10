# marmot-cli Implementation Plan

## Phase 1: Identity Persistence + Storage
- [x] Generate Nostr identity (done)
- [ ] Add SQLite storage layer using mdk-sqlite-storage
- [ ] Add identity directory (`~/.local/share/marmot-cli/identities/`)
- [ ] `identity create` — save to disk with name
- [ ] `identity list` — enumerate saved identities
- [ ] `identity show` — display npub for a saved identity
- [ ] Config file (`~/.config/marmot-cli/config.toml`)

## Phase 2: KeyPackage Publishing
- [ ] Wire up MDK `create_key_package_for_event`
- [ ] Add `keypackage` CLI subcommand
- [ ] `keypackage publish` — create + publish kind 30443 to relays
- [ ] `keypackage show` — display current KeyPackage info
- [ ] Relay pool integration (connect, publish, confirm)

## Phase 3: Daemon Mode + JSON-RPC
- [ ] Tokio-based daemon with Unix socket listener
- [ ] JSON-RPC 2.0 server (methods: send_message, list_groups, create_group, etc.)
- [ ] Background relay subscription (notifications loop)
- [ ] `daemon` subcommand starts the daemon
- [ ] `daemon status` / `daemon stop` helpers

## Phase 4: Group Lifecycle
- [ ] `group create --name <name>` — create MLS group
- [ ] `group invite --group <id> --member <npub>` — send Welcome
- [ ] `group list` — show joined groups
- [ ] `group join --welcome <event_id>` — accept pending Welcome
- [ ] `send --group <id> <message>` — send encrypted message
- [ ] `receive` / `messages --group <id>` — poll/display messages

## Phase 5: Tooling + CI
- [ ] `justfile` with check, test, precommit recipes
- [ ] `.github/workflows/ci.yml` — fmt, clippy, test, build
- [ ] CHANGELOG.md per crate
- [ ] AGENTS.md for AI coding agent conventions
