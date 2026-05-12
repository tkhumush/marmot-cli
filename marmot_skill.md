# marmot_skill.md — How to Use marmot-cli

> Written for AI agents. Tells you everything you need to drive marmot-cli as a subprocess.

marmot-cli is a headless Marmot Protocol messaging agent. It sends and receives end-to-end encrypted messages over Nostr relays using MLS (RFC 9420) encryption. Designed to be driven by AI agents via subprocess or JSON-RPC daemon, like signal-cli.

---

## Quick Mental Model

- **Identity** = a Nostr keypair stored on disk. One is the "default" used by all commands.
- **KeyPackage** = a published credential that lets others invite you to a group. Must be published before anyone can DM or invite you. Also auto-publishes your inbox relay list (kind 10050) on first publish.
- **Group** = an MLS-encrypted conversation. Every DM is also a group (2 members, empty name).
- **nostr-group-id** = the 32-byte hex h-tag that identifies a group on relays. This is what `--group` flags take.
- **Daemon** = long-running process that keeps a live WebSocket subscription to relays. New messages and invitations arrive automatically — no manual `receive` needed.

---

## Setup (first time)

```bash
# 1. Create an identity
marmot-cli identity create --name myagent

# 2. Set it as the default (all other commands use the default)
marmot-cli identity set-default myagent

# 3. Publish your KeyPackage — REQUIRED before anyone can invite or DM you
#    Also auto-publishes inbox relay list (kind 10050) and key-package relay list (kind 10051)
marmot-cli keypackage publish
```

After this your agent is reachable. Share your npub (shown in `identity list`) with anyone who wants to contact you.

---

## Identity Commands

```bash
marmot-cli identity create [--name <name>]   # create new keypair
marmot-cli identity list                      # list all saved identities; shows which is default
marmot-cli identity show <name>               # show npub + nsec for an identity
marmot-cli identity set-default <name>        # set default identity for all commands
marmot-cli identity delete <name>             # remove identity files from disk
```

**Note:** Deleting an identity removes key files but the MLS group state in the database persists. Old groups from deleted identities will still appear in `dm list` / `groups list` but can no longer send valid messages.

---

## KeyPackage Commands

```bash
marmot-cli keypackage publish                # generate kind 30443 and publish to default relays
                                             # auto-publishes kind 10050 + 10051 relay lists if not set
marmot-cli keypackage show                   # fetch our current KeyPackage from relays
marmot-cli keypackage check <npub>           # check if a user has a valid KeyPackage on relays
marmot-cli keypackage list                   # list all our kind 30443 events on relays
marmot-cli keypackage delete <event_id>      # delete a specific KeyPackage event (kind 5 deletion)
marmot-cli keypackage delete-all --confirm   # delete ALL our KeyPackages from relays
```

**`keypackage check` output to parse:**
```
KeyPackage found — user is reachable.
  event ID: <hex>
  created:  <timestamp>
```
or:
```
No KeyPackage found for <npub>.
  They need to run 'keypackage publish' before you can DM or invite them.
```

---

## Direct Messages

### Starting a DM

```bash
marmot-cli dm create --recipient <npub> [--publish]
```

- Fetches the recipient's KeyPackage from relays
- Creates a 2-member MLS group (empty name, so White Noise shows it as a DM)
- `--publish` sends the MLS commit to relays AND delivers the welcome gift-wrap to the recipient's inbox relays (kind 10050) with NIP-42 auth
- **Deduplication**: if a DM with this recipient already exists locally, prints the existing group ID and exits

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
  '<DM with npub1abc...>' (nostr-id: <32-byte-hex>)
```
The `nostr-id` value is what you pass to `--group` in all subsequent commands.

### Sending a message

```bash
marmot-cli dm send --group <nostr-group-id-hex> --message "hello" [--publish] [--reply-to <event_id>]
```

- `--publish` sends to relays
- `--reply-to <event_id>` includes an `e` tag for thread replies

**Output to parse:**
```
Encrypted message created!
  Event ID: <hex>
  Kind: 445
  Publishing to relays...
  Published: 3/3 relays OK
```

### Reading messages

```bash
marmot-cli dm messages --group <hex> [--limit 20] [--before <unix-ts>] [--after <unix-ts>]
```

Output (newest first):
```
Messages in '<DM with npub1...>' (newest first):
  [<unix-timestamp>] npub1abc...: message content here
  (1 messages)
```

`--before` / `--after` filter by Unix timestamp for cursor-based pagination.

---

## Groups

### Create a named group

```bash
marmot-cli groups create --name "my-team" [--description "optional"] [--member <npub> ...] [--publish]
```

- `--member <npub>` can be repeated to invite initial members in one step

### Show group details

```bash
marmot-cli groups show --group <hex>
```

Shows members (with admin/you markers), relay list, and nostr-id.

### Invite someone

```bash
marmot-cli groups invite --group <hex> --member <npub> [--publish]
```

### Send a message

```bash
marmot-cli groups send --group <hex> --message "hello" [--publish] [--reply-to <event_id>]
```

### List / members / messages

```bash
marmot-cli groups list
marmot-cli groups members --group <hex>
marmot-cli groups messages --group <hex> [--limit 20] [--before <unix-ts>] [--after <unix-ts>]
```

### Pending invitations

```bash
marmot-cli groups pending          # list received welcome messages not yet accepted
marmot-cli groups join             # accept all pending welcomes (no --publish recommended)
marmot-cli groups accept --group <hex>   # accept one specific invitation
marmot-cli groups decline --group <hex>  # decline one specific invitation
```

### Admin operations

```bash
marmot-cli groups rename --group <hex> --name "new-name" [--publish]
marmot-cli groups remove-members --group <hex> --member <npub> [--member <npub> ...] [--publish]
marmot-cli groups promote   --group <hex> --member <npub> [--publish]
marmot-cli groups demote    --group <hex> --member <npub> [--publish]
marmot-cli groups self-demote --group <hex> [--publish]   # required before leaving if admin
marmot-cli groups leave     --group <hex> [--publish]
```

---

## Messages (Reactions, Deletion, Search)

```bash
marmot-cli messages react   --group <hex> --event-id <msg_event_id> [--emoji "+"] [--publish]
marmot-cli messages delete  --group <hex> --event-id <msg_event_id> [--publish]
marmot-cli messages search  --group <hex> "query" [--limit 50]
marmot-cli messages search-all "query" [--limit 20]
```

`react` sends a kind 7 reaction (default `+`). `delete` sends a kind 5 deletion request. Both are MLS-encrypted inner rumors.

---

## Receiving Messages

### Option A — Daemon (recommended, always live)

```bash
marmot-cli daemon [--listen 127.0.0.1:9222]
```

Keeps a persistent WebSocket subscription to relays. New messages and group invitations are pushed by the relay and processed automatically — no polling needed. See the **Daemon Mode** section for RPC details.

### Option B — Manual receive

```bash
marmot-cli receive [--limit 50] [--offline]
```

One-shot fetch: connects to relays, fetches all group events and gift wraps, decrypts and stores them.

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
```

Use `--offline` to skip relay fetch and only process already-stored data.

**Agent polling loop (if not using daemon):**
```bash
while true; do
  marmot-cli receive --limit 100
  sleep 30
done
```

---

## Chats (Unified Conversation View)

```bash
marmot-cli chats list [--limit 50]
```

Shows all DMs and groups with last-message preview, sorted by most recent activity.

**Output:**
```
Conversations (3):
  '<DM with npub1abc...>' (3e8f32da...)
    [1748000000] npub1abc...: hey how are you…
  'team-alpha' (7f2c19ab...)
    [1747990000] npub1xyz...: meeting at 3pm
```

---

## Profile

```bash
marmot-cli profile show
marmot-cli profile update [--name <n>] [--display-name <n>] [--about <text>] [--picture <url>] [--nip05 <id>] [--lud16 <addr>]
```

`show` fetches our kind 0 from relays. `update` merges the given fields with the existing profile and republishes kind 0.

---

## Users

```bash
marmot-cli users show <npub>       # fetch any user's kind 0 profile from relays
marmot-cli users search "query"    # search your follows by name / display_name / about
```

`search` batch-fetches profiles for all accounts you follow and filters case-insensitively.

---

## Follows

```bash
marmot-cli follows list             # show our kind 3 contact list
marmot-cli follows add <npub>       # add to contact list and republish
marmot-cli follows remove <npub>    # remove from contact list and republish
```

---

## Block List

```bash
marmot-cli block list               # show kind 10000 mute list (public p-tags)
marmot-cli block add <npub>         # add to block list and republish
marmot-cli block remove <npub>      # remove from block list and republish
```

Uses public p-tags in kind 10000 (NIP-51 mute list). Block status is advisory — it is not enforced by marmot-cli itself.

---

## Relay Commands

```bash
marmot-cli relay list                              # show default, inbox (10050), and NIP-65 (10002) relays
marmot-cli relay add <wss://...> [--type inbox]    # add to inbox relay list (default)
marmot-cli relay add <wss://...> --type nip65      # add to NIP-65 relay list (kind 10002)
marmot-cli relay remove <wss://...> [--type inbox]
marmot-cli relay remove <wss://...> --type nip65
```

`--type inbox` (default) manages kind 10050 — where others deliver gift-wrap welcome events to you.
`--type nip65` manages kind 10002 — your general read/write relay preferences.

Default relays (built-in, always used):
- `wss://nos.lol`
- `wss://relay.primal.net`
- `wss://relay.damus.io`

---

## Daemon Mode

```bash
marmot-cli daemon [--listen 127.0.0.1:9222]
```

Starts two things in parallel:
1. **JSON-RPC TCP server** — one JSON object per line, accepts RPC calls
2. **Live WebSocket subscription** — persistent connection to all default relays; pushes new events to local storage in real-time

**RPC wire format:**
```bash
echo '{"jsonrpc":"2.0","method":"ping","id":1}' | nc 127.0.0.1 9222
# → {"jsonrpc":"2.0","result":{"pong":true},"id":1}
```

**Available RPC methods:**

| Method | Params | Returns |
|---|---|---|
| `ping` | — | `{"pong": true}` |
| `identity_npub` | — | `{"npub": "npub1..."}` |
| `list_groups` | — | `{"groups": [{"nostr_id": "<hex>", "name": "<str>"}]}` |
| `send_message` | `group_id`, `content`, `publish` | `{"sent": true, "event_id": "<hex>", "published": bool}` |
| `receive` | — | `{"new_messages": N, "new_welcomes": N}` |

**Example session:**
```bash
echo '{"jsonrpc":"2.0","method":"identity_npub","id":1}' | nc 127.0.0.1 9222
echo '{"jsonrpc":"2.0","method":"list_groups","id":2}' | nc 127.0.0.1 9222
echo '{"jsonrpc":"2.0","method":"send_message","params":{"group_id":"<hex>","content":"hello","publish":true},"id":3}' | nc 127.0.0.1 9222
```

**Live subscription:** When the daemon is running, relay-pushed events are automatically decrypted and stored. Any subsequent `dm messages` / `groups messages` call returns the latest data without a manual `receive`.

---

## Common Agent Workflows

### Workflow: Send a DM to a known contact

```bash
# Step 1: ensure DM group exists (idempotent)
marmot-cli dm create --recipient <npub> --publish

# Step 2: find the group ID
marmot-cli dm list   # parse nostr-id from '<DM with <npub>>' line

# Step 3: send
marmot-cli dm send --group <hex> --message "Hello!" --publish
```

### Workflow: Run as a persistent agent (daemon)

```bash
# Terminal 1 — start daemon (messages arrive automatically)
marmot-cli daemon

# Terminal 2 — read messages any time, no receive needed
marmot-cli dm messages --group <hex> --limit 10
marmot-cli chats list
```

### Workflow: Accept all pending invitations

```bash
marmot-cli receive         # fetch gift-wraps (skip if daemon is running)
marmot-cli groups pending  # see what came in
marmot-cli groups join     # accept all (no --publish)
marmot-cli keypackage publish  # rotate consumed KeyPackage
```

### Workflow: Invite someone to a group

```bash
marmot-cli groups create --name "team-alpha" --member npub1... --publish
# or step by step:
marmot-cli groups create --name "team-alpha" --publish
marmot-cli groups list   # get nostr-id
marmot-cli groups invite --group <hex> --member <npub> --publish
```

### Workflow: Leave a group gracefully

```bash
marmot-cli groups self-demote --group <hex> --publish   # if admin
marmot-cli groups leave --group <hex> --publish
```

### Workflow: Check if a user is reachable before DMing

```bash
marmot-cli keypackage check <npub>
# "found — user is reachable" → proceed with dm create
# "No KeyPackage found" → they need to run keypackage publish
```

---

## Output Format & Parsing

All structured data is on labeled lines. Errors go to stderr. Exit 0 = success.

**Parsing nostr-group-id from `dm list`:**
```
'<DM with npub1abc...xyz>' (nostr-id: 3e8f32dae307...)
```
Extract the value after `nostr-id: ` and before `)`.

**Parsing messages from `dm messages` / `groups messages`:**
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

**Parsing receive summary:**
```
  2 new message(s)
  1 new invitation(s)
```

---

## Important Caveats

**Relay latency:** One-shot fetch functions wait up to 8 seconds for EOSE. Daemon subscription events arrive in <1s after relay propagation.

**Database is shared across identities.** Old MLS group state from deleted identities stays in the database. `dm list` and `groups list` show ALL groups from ALL past identities. Groups from deleted identities are stale — messages sent through them will be signed with the wrong key.

**DM deduplication:** `dm create --recipient <npub>` checks for an existing DM (same 2 members, empty name) and reuses it.

**The `--group` flag takes the nostr-group-id (h-tag hex), not the MLS group ID.** Always use the hex shown in `dm list` / `groups list`.

**Empty group name = DM.** White Noise distinguishes DMs from named groups by `name == ""`. Never set a name on a DM group — it will appear as a named group with admin badges in other clients.

**Inner rumor kind must be 9 (ChatMessage).** All chat messages inside MLS groups use kind 9 per MIP-03. Kind 1 decrypts but is silently ignored by White Noise.

**Do not run self-update after joining.** Running a self-update commit immediately after accepting a welcome causes epoch ordering issues — the other side's replies fail to decrypt. `groups join` does NOT run self-update for this reason.

**Gift-wrap delivery uses inbox relays.** Welcome events (kind 1059) are delivered to the recipient's kind 10050 inbox relays with NIP-42 auth, falling back to DEFAULT_RELAYS if none published.

**Admin must self-demote before leaving.** `groups leave` errors if you are still an admin. Run `groups self-demote --group <hex> --publish` first.

**KeyPackages are single-use.** After accepting a group welcome, your KeyPackage is consumed. `groups join` automatically rotates your KeyPackage. If you call `groups accept` manually, run `keypackage publish` afterward.

**Daemon subscription has no h-tag filter.** The live subscription receives all kinds 445/10449/4459 from connected relays; events for groups you don't belong to are silently rejected by `process_incoming_event`. This ensures you receive events for groups joined during the daemon session without a restart.

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

## Common Errors

| Error | Cause | Fix |
|---|---|---|
| `No default identity set.` | No default configured | `identity set-default <name>` |
| `No KeyPackage found for <npub>` | Recipient hasn't published | Ask them to run `keypackage publish` |
| `Group with nostr id '...' not found locally` | Wrong ID or not yet received | Check `groups list`; run `receive` |
| `add member failed: InviteeMissingRequiredProposal` | Stale/malformed KeyPackage | Ask them to `keypackage delete-all --confirm` then republish |
| `leave group failed: ... admin ...` | Still listed as admin | Run `groups self-demote --group <hex> --publish` first |
