# SKILL.md — How to Use marmot-cli

> Written for AI agents. Tells you everything you need to drive marmot-cli as a subprocess.

marmot-cli is a headless Marmot Protocol messaging agent. It sends and receives end-to-end encrypted messages over Nostr relays using MLS (RFC 9420) encryption. It is designed to be driven by AI agents via subprocess, like signal-cli.

---

## Quick Mental Model

- **Identity** = a Nostr keypair stored on disk. One is the "default" used by all commands.
- **KeyPackage** = a published credential that lets others invite you to a group. Must be published before anyone can DM or invite you.
- **Group** = an MLS-encrypted conversation. Every DM is also a group (2 members, empty name).
- **nostr-group-id** = the 32-byte hex h-tag that identifies a group on relays. This is what `--group` flags take.
- **Receive** = explicitly fetch + decrypt new messages from relays. Nothing arrives automatically without calling this.

---

## Setup (first time)

```bash
# 1. Create an identity
marmot-cli identity create --name myagent

# 2. Set it as the default (all other commands use the default)
marmot-cli identity set-default myagent

# 3. Publish your KeyPackage — REQUIRED before anyone can invite or DM you
marmot-cli keypackage publish
```

After this your agent is reachable. Share your npub (shown in `identity list`) with anyone who wants to contact you.

---

## Identity Commands

```bash
marmot-cli identity create [--name <name>]   # create new keypair; name defaults to "default"
marmot-cli identity list                      # list all saved identities; shows which is default
marmot-cli identity show <name>               # show npub + nsec for an identity
marmot-cli identity set-default <name>        # set default identity for all commands
marmot-cli identity delete <name>             # remove identity files from disk (does NOT clear DB)
```

**Important:** Deleting an identity removes the key files but the MLS group state in the database persists. If you create a new identity, old groups from deleted identities remain in the database and will appear in `dm list` / `groups list`. This is expected — those old groups can no longer send valid messages.

---

## KeyPackage Commands

```bash
marmot-cli keypackage publish   # generate kind 30443 and publish to default relays
marmot-cli keypackage show      # fetch our current KeyPackage from relays (confirms it's live)
```

You must publish a KeyPackage before:
- Anyone can send you a DM
- Anyone can invite you to a group

Republish periodically if your agent runs for a long time — key packages can be consumed by group creation.

---

## Direct Messages

### Starting a DM

```bash
marmot-cli dm create --recipient <npub> [--publish]
```

- Fetches the recipient's KeyPackage from relays
- Creates a 2-member MLS group
- `--publish` sends the MLS commit event to relays AND delivers the welcome gift-wrap to the recipient's inbox relays (kind 10050) with NIP-42 auth
- **Deduplication**: if a DM with this recipient already exists locally, it prints the existing group ID and exits — it does NOT create a duplicate

**Output to parse:**
```
DM group created!
  Commit event ID: <hex>
  Welcome rumors: 1
  ...
  evolution_commit: 3/3 relays OK
  welcome (gift wrap): 2/2 relays OK
```

Or if already exists:
```
DM with this recipient already exists — reusing it.
  nostr-id: <hex>
```

### Finding the group ID

```bash
marmot-cli dm list
```

Output:
```
Conversations:
  '<DM with npub1...>' (nostr-id: <32-byte-hex>)
  'my-group-name'      (nostr-id: <32-byte-hex>)
```

The `nostr-id` value is what you pass to `--group` in all subsequent commands.

### Sending a message

```bash
marmot-cli dm send --group <nostr-group-id-hex> --message "hello" [--publish]
```

- Creates an MLS application message (kind 445, inner rumor kind 9 per MIP-03)
- `--publish` sends to relays (default relays + group-configured relays)
- Without `--publish`, the event is created and stored locally only

**Output to parse:**
```
Encrypted message created!
  Event ID: <hex>
  Kind: 445
  Publishing to relays...
  Published: 3/3 relays OK
    OK wss://nos.lol
    OK wss://relay.primal.net
    OK wss://relay.damus.io
```

### Reading messages

```bash
marmot-cli dm messages --group <hex> [--limit 20]
```

Output (newest first):
```
Messages in '<DM with npub1...>' (newest first):
  [<unix-timestamp>] npub1abc...: message content here
  [<unix-timestamp>] npub1def...: another message
  (2 messages)
```

**Important:** `dm messages` only shows messages already stored locally. Run `receive` first to fetch new messages from relays.

---

## Groups

### Create a named group

```bash
marmot-cli groups create --name "my-team" [--publish]
```

Creates a 1-member MLS group. Use `groups invite` to add others.

### Invite someone

```bash
marmot-cli groups invite --group <hex> --member <npub> [--publish]
```

Fetches the member's KeyPackage from relays, adds them via MLS add-member commit, and sends them a welcome gift-wrap to their inbox relays.

### List groups

```bash
marmot-cli groups list
```

### Show members

```bash
marmot-cli groups members --group <hex>
```

### Read messages

```bash
marmot-cli groups messages --group <hex> [--limit 20]
```

### Check pending invitations

```bash
marmot-cli groups pending
```

Shows invitations (welcome messages) received but not yet accepted.

### Accept all pending invitations

```bash
marmot-cli groups join [--publish]
```

Accepts all pending welcomes. `--publish` sends a self-update commit event to relays after joining. **Do not run self-update automatically** — it can cause epoch ordering issues. Only publish if you explicitly need key rotation.

---

## Receiving Messages

```bash
marmot-cli receive [--limit 50] [--offline]
```

This is the core polling command. It:
1. Fetches kind 445/10449/4459 events for all known group h-tags from relays
2. Fetches kind 1059 gift-wrap events (welcome invitations) addressed to our pubkey
3. Decrypts and stores everything in the local SQLite database
4. Prints a summary

**Output to parse:**
```
Checking 5 known group(s)...
  Fetched 12 group event(s) from relays.
Checking for group invitations (gift wraps)...
  Fetched 3 gift-wrap event(s) from relays.
Done.
  2 new message(s)
  1 new invitation(s)
  10 event(s) skipped (already processed or unrecognised)

Run 'groups messages --group <id>' or 'dm messages --group <id>' to read.
```

`--offline`: skip relay fetch, only process already-stored data.

**Agent polling loop:**
```bash
while true; do
  marmot-cli receive --limit 100
  marmot-cli dm messages --group <hex> --limit 5
  sleep 30
done
```

---

## Relay Commands

```bash
marmot-cli relay list   # show default relays
```

Default relays (inherited from White Noise):
- `wss://nos.lol`
- `wss://relay.primal.net`
- `wss://relay.damus.io`

---

## Daemon Mode

```bash
marmot-cli daemon [--listen 127.0.0.1:9222]
```

Starts a JSON-RPC TCP server. Wire format: one JSON object per line.

**Currently live:**
```bash
echo '{"jsonrpc":"2.0","method":"ping","id":1}' | nc 127.0.0.1 9222
# → {"jsonrpc":"2.0","result":"pong","id":1}
```

All other methods (`identity_npub`, `list_groups`, `send_message`) are stubs — they return placeholder responses. Full daemon implementation is in progress (see `docs/HANDOFF.md` Phase 6).

---

## Common Agent Workflows

### Workflow: Send a DM to a known contact

```bash
# Step 1: ensure DM group exists (idempotent — reuses if already exists)
marmot-cli dm create --recipient <npub> --publish

# Step 2: find the group ID
marmot-cli dm list
# parse: '<DM with <npub>>' → nostr-id: <hex>

# Step 3: send message
marmot-cli dm send --group <hex> --message "Hello!" --publish
```

### Workflow: Poll and read new messages

```bash
# Fetch from relays and store
marmot-cli receive --limit 100

# Read DM messages
marmot-cli dm messages --group <hex> --limit 20

# Read group messages
marmot-cli groups messages --group <hex> --limit 20
```

### Workflow: Accept all pending invitations

```bash
marmot-cli receive         # fetch gift-wraps first
marmot-cli groups pending  # see what came in
marmot-cli groups join     # accept all (no --publish unless you need key rotation)
```

### Workflow: Invite someone to a group

```bash
# 1. Create the group
marmot-cli groups create --name "team-alpha" --publish

# 2. Find the group ID
marmot-cli groups list

# 3. Invite a member
marmot-cli groups invite --group <hex> --member <npub> --publish
```

---

## Output Format & Parsing

All structured data is on labeled lines. Errors go to stderr. Exit 0 = success.

**Parsing the nostr-group-id from `dm list`:**
```
'<DM with npub1abc...xyz>' (nostr-id: 3e8f32dae307...)
```
Extract the value after `nostr-id: ` and before `)`.

**Parsing messages from `dm messages`:**
```
  [1778449494] npub1abc...def: message content here
```
Format: `[<unix-ts>] <npub-prefix>: <content>`

**Parsing relay results from `--publish` commands:**
```
  Published: 3/3 relays OK
    OK wss://nos.lol
    FAIL wss://relay.damus.io
```
`N/M relays OK` — N succeeded out of M total.

**Parsing receive summary:**
```
  2 new message(s)
  1 new invitation(s)
```
Use these counts to decide whether to read messages or check pending invitations.

---

## Important Caveats

**Relay latency:** Publishing takes 1-2 seconds; fetching waits up to 8 seconds for EOSE. Factor this into timeouts.

**No auto-receive:** Messages do not arrive automatically. You must call `receive` on a schedule.

**Database is shared across identities.** When you delete an identity and create a new one, old MLS group state stays in the database. `dm list` and `groups list` show ALL groups from ALL past identities. Groups from deleted identities are stale — `dm send` will appear to succeed but the message will be signed with the wrong key.

**DM deduplication:** `dm create --recipient <npub>` checks for an existing DM (same 2 members, empty name) and reuses it. If you see "already exists — reusing it," use the printed nostr-id.

**The `--group` flag takes the nostr-group-id (h-tag hex), not the MLS group ID.** These are different. Always use the hex shown in `dm list` / `groups list`.

**Empty group name = DM.** This is how White Noise (and marmot-cli) distinguish DMs from named groups. Never set a group name when creating a DM — it will appear as a named group with admin in other clients.

**Inner rumor kind must be 9 (ChatMessage).** All chat messages inside MLS groups use kind 9 per MIP-03. Kind 1 (TextNote) decrypts but is silently ignored by White Noise.

**Do not run self-update after joining.** Per MIP-02 / White Noise convention, running a self-update commit immediately after accepting a welcome causes epoch ordering issues — the other side's replies fail to decrypt. Only do self-update for periodic key rotation, not as part of the join flow.

**Gift-wrap delivery uses inbox relays.** Welcome events (kind 1059) are delivered to the recipient's kind 10050 inbox relays with NIP-42 auth, not to the sender's default relays. If a recipient has no kind 10050 relay list, delivery falls back to DEFAULT_RELAYS.

---

## State Files

All state is at `~/.local/share/marmot-cli/`:

| Path | Contents |
|---|---|
| `identities/<name>.json` | Public key + metadata |
| `identities/<name>.nsec` | Secret key (mode 0600) |
| `marmot.db` | Encrypted SQLite — all MLS group state, messages, welcomes |
| `db.key` | AES encryption key for marmot.db (mode 0600) |
| `config.json` | Default identity name |

---

## Exit Codes & Error Handling

- Exit 0: success
- Exit non-zero or error on stderr: something failed

Errors are printed to stderr. Check stderr before acting on stdout.

Common errors:
- `No default identity set.` → run `identity set-default <name>`
- `No KeyPackage found for <npub>` → recipient hasn't published a KeyPackage; they need to run `keypackage publish`
- `Group with nostr id '...' not found locally` → wrong group ID or you need to `receive` first
- `DM creation failed: add member failed: InviteeMissingRequiredProposal` → recipient's KeyPackage is missing required MLS proposals; they need to republish
