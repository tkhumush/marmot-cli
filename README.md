# marmot-cli

**A headless Marmot messaging agent, inspired by signal-cli.**

This is a command-line tool and daemon that implements the [Marmot Protocol](https://github.com/marmot-protocol/marmot) for end-to-end encrypted group messaging over the [Nostr](https://github.com/nostr-protocol/nostr) relay network. It is designed first and foremost as a communication layer for AI agents, headless scripts, and automation workflows — but is equally useful for anyone who wants a `signal-cli`-style experience for Marmot.

## What is Marmot?

Marmot combines [MLS (Messaging Layer Security, RFC 9420)](https://www.rfc-editor.org/rfc/rfc9420.html) with Nostr's decentralized relay network to deliver private, scalable group messaging without phone numbers, centralized servers, or gatekeepers.

## Why marmot-cli?

- **Headless by design** — daemon mode, JSON-RPC API, stdout/stdout
- **Agent-first** — built for Hermes, OpenClaw, cron jobs, and any future agent
- **No GUI** — runs on servers, VPSs, embedded systems, anywhere Rust compiles
- **Decentralized identity** — Nostr keys, not phone numbers or corporate accounts
- **Pluggable storage** — in-memory for testing, SQLite for persistence
- **True ownership** — your keys, your relays, your data

## Features

- [x] Generate and manage Nostr identities
- [x] Publish and discover KeyPackages on relays
- [x] Create and join encrypted groups
- [x] Send and receive messages
- [x] Daemon mode with JSON-RPC / gRPC interfaces
- [ ] Hermes plugin integration
- [ ] Python FFI bindings
- [ ] Advanced relay management

## Quick Start

```bash
# Clone the repository
git clone https://github.com/tkhumush/marmot-cli.git
cd marmot-cli

# Install the CLI
cargo install --path crates/marmot-agent-cli

# Generate an identity
marmot-cli identity create --name "my-agent"

# Publish your KeyPackage
marmot-cli keypackage publish

# Start the daemon
marmot-cli daemon --listen /tmp/marmot-agent.sock

# In another terminal, send a message
marmot-cli send --group <GROUP_ID> "Hello from marmot-cli!"
```

## Architecture

```
marmot-agent-cli        ← User-facing CLI tool
├── marmot-agent-rpc    ← JSON-RPC / gRPC server
├── marmot-agent-core   ← Identity, groups, messages, relay manager
│   └── mdk-core        ← (external) Marmot Development Kit (MLS + Nostr)
└── marmot-agent-ffi    ← Python bindings for agent integration
```

## Default Relays

The default relay list is inherited from the [White Noise](https://github.com/marmot-protocol/whitenoise) messenger:

- `wss://nos.lol`
- `wss://relay.primal.net`
- `wss://relay.damus.io`

These are configurable at runtime.

## License

[MIT](LICENSE) — if people like it, let them use it.