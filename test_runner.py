#!/usr/bin/env python3
"""Drive marmot-cli as a subprocess — full send/receive round-trip test."""

import subprocess, re, time, os

BINARY = os.path.abspath("./target/release/marmot-cli")
CWD = "/home/taymur/projects/marmot-cli"

def run(cmd_args, timeout=30):
    full = [BINARY] + cmd_args
    result = subprocess.run(full, capture_output=True, text=True, timeout=timeout, cwd=CWD)
    # Strip tracing lines
    clean = re.sub(r'^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z\s+\w+\s+.*$', '', result.stdout, flags=re.MULTILINE)
    clean = "\n".join(line for line in clean.splitlines() if line.strip())
    return clean, result.stderr, result.returncode

def set_default(identity):
    out, err, rc = run(["identity", "set-default", identity])
    return rc == 0

def list_groups():
    out, _, rc = run(["groups", "list"])
    groups = []
    if rc == 0:
        for line in out.splitlines():
            # e.g. "  Group 'name' (nostr-id: abc123...)"
            m = re.search(r"\(nostr-id:\s*([a-f0-9]+)\)", line)
            if m:
                name = re.search(r"Group\s+'([^']+)'", line)
                groups.append({
                    "name": name.group(1) if name else "unnamed",
                    "id": m.group(1)
                })
    return groups

def list_pending():
    out, _, rc = run(["groups", "pending"])
    if "No pending group invitations" in out:
        return []
    pending = []
    for line in out.splitlines():
        m = re.search(r"nostr group:\s*([a-f0-9]+)", line)
        if m:
            pending.append(m.group(1))
    return pending

def receive_messages(limit=50):
    out, _, rc = run(["receive", "--limit", str(limit)])
    if rc != 0:
        print(f"[ERR] receive failed: {out}")
        return None
    stats = {}
    m = re.search(r'(\d+) new message\(s\)', out)
    stats["new_messages"] = int(m.group(1)) if m else 0
    m = re.search(r'(\d+) MLS commit\(s\) applied', out)
    stats["commits"] = int(m.group(1)) if m else 0
    m = re.search(r'(\d+) new group invitation\(s\) received', out)
    stats["new_invites"] = int(m.group(1)) if m else 0
    m = re.search(r'(\d+) event\(s\) skipped', out)
    stats["skipped"] = int(m.group(1)) if m else 0
    return stats

def join_groups(publish=True):
    args = ["groups", "join"]
    if publish:
        args.append("--publish")
    out, _, rc = run(args)
    return rc == 0, out

def get_dm_messages(group_id_hex, limit=20):
    out, _, rc = run(["dm", "messages", "--group", group_id_hex, "--limit", str(limit)])
    messages = []
    if rc == 0:
        for line in out.splitlines():
            m = re.match(r"\s*\[([^\]]+)\]\s+(\S+):\s+(.*)", line)
            if m:
                messages.append({
                    "time": m.group(1),
                    "sender": m.group(2),
                    "content": m.group(3)
                })
    return messages

def send_dm(group_id_hex, text, publish=True):
    args = ["dm", "send", "--group", group_id_hex, "--message", text]
    if publish:
        args.append("--publish")
    out, _, rc = run(args)
    if rc != 0:
        print(f"[ERR] send_dm failed: {out}")
        return False
    ok = re.search(r'(\d+)/(\d+) relays OK', out)
    if ok:
        print(f"    published {ok.group(1)}/{ok.group(2)} relays")
    return True

def get_first_dm_conversation_with(recipient_npub_substring):
    groups = list_groups()
    for g in groups:
        if g["name"].startswith("dm:") and recipient_npub_substring in g["name"]:
            return g["id"]
    return None

def test_roundtrip():
    print("=" * 60)
    print("MARMOT-CLI ROUND-TRIP TEST")
    print("=" * 60)

    B_NPUB = "npub1vl73xzhpyucxjt5dvam2zyfsllffc4kzwdn9rppym3ck5twpedlsamyt49"

    # --- A: publish keypackage ---
    print("\n[1] A set-default test-agent")
    set_default("test-agent")
    print("[2] A publish keypackage")
    out, _, rc = run(["keypackage", "publish"])
    print("    OK" if rc == 0 else f"    FAIL: {out}")

    # --- B: publish keypackage ---
    print("\n[3] B set-default test-agent-2")
    set_default("test-agent-2")
    print("[4] B publish keypackage")
    out, _, rc = run(["keypackage", "publish"])
    print("    OK" if rc == 0 else f"    FAIL: {out}")

    # --- A creates DM to B ---
    print("\n[5] A set-default, create DM to B")
    set_default("test-agent")
    out, _, rc = run(["dm", "create", "--recipient", B_NPUB, "--publish"])
    if rc != 0:
        print(f"    FAIL: {out}")
        return
    print("    DM created and published")

    print("[6] Sleep 5s for relay propagation")
    time.sleep(5)

    # --- B: receive → pending → join ---
    print("\n[7] B set-default, receive")
    set_default("test-agent-2")
    stats = receive_messages()
    print(f"    {stats}")

    print("[8] B list pending")
    pending = list_pending()
    print(f"    pending: {pending}")

    if pending:
        print("[9] B join --publish")
        ok, out = join_groups(publish=True)
        print(f"    join ok={ok}")
    else:
        print("[9] No pending groups to join")

    # B: read messages
    gid_b = get_first_dm_conversation_with(B_NPUB)
    print(f"[10] B DM group id: {gid_b}")
    if gid_b:
        msgs = get_dm_messages(gid_b, limit=10)
        print(f"[11] B reads ({len(msgs)} messages):")
        for m in msgs:
            print(f"      [{m['time']}] {m['sender']}: {m['content']}")

        # B replies
        print(f"[12] B send reply")
        send_dm(gid_b, "Hello back from B!", publish=True)
    else:
        print("[WARN] B could not find DM conversation group")

    # --- A: receive → read ---
    print("\n[13] Sleep 3s")
    time.sleep(3)
    print("[14] A set-default, receive")
    set_default("test-agent")
    stats = receive_messages()
    print(f"    {stats}")

    gid_a = get_first_dm_conversation_with("npub1vl73xzhpyucxjt5dvam2zyfsllffc4kzwdn9rppym3ck5twpedlsamyt49")
    print(f"[15] A DM group id: {gid_a}")
    if gid_a:
        msgs = get_dm_messages(gid_a, limit=10)
        print(f"[16] A reads ({len(msgs)} messages):")
        for m in msgs:
            print(f"      [{m['time']}] {m['sender']}: {m['content']}")

        # A replies
        print(f"[17] A send reply")
        send_dm(gid_a, "Roger that from A", publish=True)
    else:
        print("[WARN] A could not find DM conversation group")

    # --- B: final receive ---
    print("\n[18] Sleep 3s")
    time.sleep(3)
    print("[19] B set-default, receive")
    set_default("test-agent-2")
    stats = receive_messages()
    print(f"    {stats}")

    gid_b = get_first_dm_conversation_with(B_NPUB)
    if gid_b:
        msgs = get_dm_messages(gid_b, limit=10)
        print(f"[20] B final read ({len(msgs)} messages):")
        for m in msgs:
            print(f"      [{m['time']}] {m['sender']}: {m['content']}")

    print("\n" + "=" * 60)
    print("TEST COMPLETE")
    print("=" * 60)

if __name__ == "__main__":
    test_roundtrip()
