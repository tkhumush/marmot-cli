# marmot-cli

**A headless Marmot messaging agent, inspired by signal-cli.**

End-to-end encrypted group messaging over Nostr relays, driven entirely from the command line. Designed for AI agents, headless scripts, and automation — but usable by anyone who wants a terminal-first experience.

Uses [MLS (RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420.html) for encryption and the [Nostr](https://github.com/nostr-protocol/nostr) relay network for message delivery. No phone numbers, no central servers, no gatekeepers.

## Status

**CLI-complete.** All core messaging flows are implemented and interoperability with [White Noise](https://github.com/marmot-protocol/whitenoise) is confirmed — two-way MLS-encrypted messaging works end-to-end. Invoke via subprocess (signal-cli style) or drive via the JSON-RPC daemon (all core methods live).

## Install

```bash
git clone https://github.com/tkhumush/marmot-cli.git
cd marmot-cli
cargo install --path crates/marmot-agent-cli
```

Requires Rust 1.75+. Tested on Linux. Secret key files and the SQLite database are stored under `~/.local/share/marmot-cli/` (XDG data dir).

## Quick Start

```bash
# 1. Create an identity
marmot-cli identity create --name alice

# 2. Set it as the default
marmot-cli identity set-default alice

# 3. Publish your KeyPackage so others can invite you
marmot-cli keypackage publish

# 4. Start a DM with someone and send a message
marmot-cli dm create --recipient <their-npub> --publish
marmot-cli dm send --group <nostr-group-id-hex> --message "hello" --publish

# 5. Fetch new messages and invitations from relays
marmot-cli receive

# 6. Read messages
marmot-cli dm messages --group <nostr-group-id-hex>
```

## Command Reference

All commands print to stdout. Errors go to stderr. Exit code 0 = success.

### Identity

```bash
marmot-cli identity create [--name <name>]   # generate a new Nostr keypair
marmot-cli identity list                      # list all saved identities
marmot-cli identity show <name>               # show npub + nsec for an identity
marmot-cli identity set-default <name>        # set the identity used by all other commands
marmot-cli identity delete <name>             # remove an identity from disk
```

All other commands use the **default identity**. Set it once with `set-default`.

### KeyPackage

```bash
marmot-cli keypackage publish   # generate + publish kind 30443 to default relays
marmot-cli keypackage show      # fetch and display our current KeyPackage from relays
```

You must publish a KeyPackage before anyone can invite you to a group or DM.

### Direct Messages

```bash
# Start a DM (creates a 2-member MLS group, sends welcome to recipient)
marmot-cli dm create --recipient <npub> [--publish]

# Send an encrypted message
marmot-cli dm send --group <nostr-group-id-hex> --message "text" [--publish]

# Read stored messages (newest first)
marmot-cli dm messages --group <nostr-group-id-hex> [--limit 20]

# List all DM conversations
marmot-cli dm list
```

`--publish` sends the event(s) to relays. Without it, the event is created and stored locally only.

The `--group` flag takes the **nostr group ID** (32-byte hex h-tag shown in `dm list` and `groups list`).

### Groups

```bash
# Create a named group
marmot-cli groups create --name "my-group" [--publish]

# Invite someone (fetches their KeyPackage from relays, sends welcome)
marmot-cli groups invite --group <hex> --member <npub> [--publish]

# List all groups
marmot-cli groups list

# List members of a group
marmot-cli groups members --group <hex>

# Read stored messages
marmot-cli groups messages --group <hex> [--limit 20]

# List pending group invitations (received but not yet accepted)
marmot-cli groups pending

# Accept all pending invitations
marmot-cli groups join
```

### Receive

```bash
marmot-cli receive [--limit 50] [--offline]
```

Fetches two kinds of events from default relays:

1. **Group messages** (kind 445/10449/4459) — encrypted messages for all known groups
2. **Gift wraps** (kind 1059, NIP-59) — welcome invitations addressed to our pubkey

Decrypts and stores everything locally. Run this before reading messages or checking pending invitations.

`--offline` skips the relay fetch and only processes already-stored data.

### Relay

```bash
marmot-cli relay list   # show default relays
```

### Daemon

```bash
marmot-cli daemon [--listen 127.0.0.1:9222]
```

Starts a JSON-RPC TCP server (newline-delimited JSON, one object per line).

**Live methods:**

| Method | Params | Returns |
|--------|--------|---------|
| `ping` | — | `{ pong: true }` |
| `identity_npub` | — | `{ npub: string }` |
| `list_groups` | — | `{ groups: [{ nostr_id, name? }] }` |
| `send_message` | `group_id`, `content`, `publish?` | `{ sent, event_id? }` |
| `receive` | — | `{ new_messages, new_welcomes }` |

The daemon is the primary integration target for AI agent frameworks (OpenClaw, etc.). It keeps a single encrypted `AgentContext` loaded in memory and exposes all messaging operations over TCP — no subprocess overhead per call, no state reload, no relay reconnect. Any language that can open a TCP socket can drive it.

**Relay publishing latency:** `send_message` publishes to relays before returning. damus.io can be slow (503s under load). Clients should use a minimum 30-second timeout for `send_message` calls.

Quick test once the daemon is running:
```bash
echo '{"jsonrpc":"2.0","method":"ping","id":1}' | nc 127.0.0.1 9222
# → {"jsonrpc":"2.0","result":{"pong":true},"id":1}

echo '{"jsonrpc":"2.0","method":"identity_npub","id":2}' | nc 127.0.0.1 9222
# → {"jsonrpc":"2.0","result":{"npub":"npub1..."},"id":2}
```

## Agent Integration

**Recommended:** Use the JSON-RPC daemon. Start it once, keep it running, and drive all messaging over TCP. This avoids per-call state reload and relay reconnect overhead.

```bash
# Start daemon
marmot-cli daemon --listen 127.0.0.1:9222

# Poll for new messages
echo '{"jsonrpc":"2.0","method":"receive","id":1}' | nc 127.0.0.1 9222

# Send a message (use ≥30s timeout — relay publishing can be slow)
echo '{"jsonrpc":"2.0","method":"send_message","id":2,"params":{"group_id":"<hex>","content":"hello","publish":true}}' | nc 127.0.0.1 9222
```

The [openclaw-marmot](https://github.com/tkhumush/openclaw-marmot) plugin uses this pattern and provides a complete reference implementation.

**Subprocess fallback (signal-cli style):** For one-shot scripts or languages without a TCP client:

```bash
marmot-cli receive --limit 100
marmot-cli dm send --group <hex> --message "agent reply" --publish
marmot-cli groups pending
marmot-cli groups join --publish
```

## Default Relays

Inherited from the [White Noise](https://github.com/marmot-protocol/whitenoise) messenger:

- `wss://nos.lol`
- `wss://relay.primal.net`
- `wss://relay.damus.io`

## Architecture

```
marmot-agent-cli        CLI entry point (clap)
marmot-agent-core       Identity, storage, relay, AgentContext (MDK wrapper)
  └── mdk-core          Marmot Development Kit — MLS groups, encryption, Nostr events
marmot-agent-rpc        JSON-RPC TCP server (ping, identity_npub, list_groups, send_message, receive)
marmot-agent-ffi        Placeholder for FFI bindings
```

State is stored at `~/.local/share/marmot-cli/`:
- `identities/` — `<name>.json` + `<name>.nsec` (mode 0600)
- `marmot.db` — encrypted SQLite (AES key at `db.key`, mode 0600)
- `config.json` — default identity name

## License

[MIT](LICENSE)
