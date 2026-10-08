#!/bin/bash
# ======================================================================
# Verify wire formats are stable against golden fixtures.
#
# Source: Verification Scripts §7; Team Prompt 1 §8.3 (the golden-fixture
# amendment procedure: "run `cargo test --test golden_fixtures --
# --ignored`, copy the output to the fixture file, commit").
#
# CommandEnvelope (crates/kernel/platform-contracts/src/command.rs) and
# DomainEventEnvelope (crates/kernel/platform-contracts/src/event.rs) are
# serde JSON wire formats; SyncMessage
# (crates/transports/sync-transport/src/message.rs) is a hand-rolled,
# frozen, byte-exact binary layout. Every one of the three is exchanged
# between replicas, clients and services that are NOT compiled together
# and NOT upgraded together — a mobile client on last month's binary must
# still be able to talk to this week's server. If a field is reordered,
# renamed, or its type silently widened, `cargo test` inside this repo
# keeps passing (a producer and consumer that share one compiled type
# always agree with themselves), while any two builds compiled at
# different times stop being able to talk to each other. That failure
# shows up in production as deserialization errors or, worse, wrong data
# quietly parsed into the wrong field — not as a red CI check. A golden
# fixture, committed to the repo and diffed byte-for-byte, is the only
# thing that turns "the format changed" into a build-time signal.
#
# tests/golden/ does not exist in this repository as of this check being
# written. This script does NOT fabricate command_v1.json, event_v1.json
# or sync_message_v1.bin by hand: a fixture invented rather than produced
# by the real serializer would lock in a wire shape nobody actually
# verified, which is worse than having no gate at all — it would pass
# forever regardless of what the real types do. Nor does a
# `--test golden_fixtures` harness that could generate them exist yet
# (grep confirms no such test target in the tree). So today this check
# fails, on purpose, and names the gap precisely rather than inventing
# a fixture to paper over it.
# ======================================================================

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FAILED=0
GOLDEN_DIR="tests/golden"

echo "Verifying wire formats against golden fixtures..."
echo ""

# ----------------------------------------------------------------------
# 1. The fixture directory must exist.
# ----------------------------------------------------------------------
echo "  [1/3] $GOLDEN_DIR exists"
if [ ! -d "$GOLDEN_DIR" ]; then
    echo "    [FAIL] $GOLDEN_DIR not found"
    FAILED=1
else
    echo "    ok"
fi

# ----------------------------------------------------------------------
# 2. The three named fixtures exist (Verification Scripts §7).
#
# command_v1.json  <- CommandEnvelope   (platform-contracts/src/command.rs)
# event_v1.json     <- DomainEventEnvelope (platform-contracts/src/event.rs)
# sync_message_v1.bin <- SyncMessage    (sync-transport/src/message.rs)
# ----------------------------------------------------------------------
echo "  [2/3] named fixtures present"
MISSING_FIXTURES=0
for f in command_v1.json event_v1.json sync_message_v1.bin; do
    if [ ! -f "$GOLDEN_DIR/$f" ]; then
        echo "    [FAIL] missing: $GOLDEN_DIR/$f"
        MISSING_FIXTURES=1
        FAILED=1
    fi
done
[ "$MISSING_FIXTURES" -eq 0 ] && echo "    ok"

# ----------------------------------------------------------------------
# 3. Fixtures that DO exist must be well-formed enough to diff against.
#
# This is a sanity check, not a substitute for the real comparison — it
# does not invoke cargo (see MACHINE CONSTRAINT: this host has 8GB RAM
# and static analysis is strongly preferred). Once fixtures exist, the
# real gate is a scoped `cargo test -p platform-contracts -p sync-transport
# --test golden_fixtures -j 2` that re-serializes the fixed sample values
# and diffs byte-for-byte against these files — that step belongs in the
# generation/verification harness itself (Team Prompt 1 §8.3), not
# duplicated here in bash.
#
# Deliberately implemented with awk/od only, NOT python3/node: this repo
# is a Rust workspace and neither interpreter is a guaranteed-present
# build dependency. An earlier version of this check shelled out to
# python3/node and treated "neither interpreter is on PATH" the same as
# "the JSON is malformed" — on a host with no node and a Microsoft Store
# python3 stub (python3 exits non-zero printing an install nag, not a
# parse error) it reported [FAIL] "not valid JSON" against fixtures that
# were in fact well-formed. That is a false positive: it fails conformant
# fixtures because a scripting runtime happened to be absent, which is
# exactly the kind of ungrounded violation report that trains people to
# ignore this gate. The awk/od replacement below has no such dependency.
# ----------------------------------------------------------------------
echo "  [3/3] existing fixtures are well-formed"

check_json_balanced() {
    # Bracket/quote-balance validator: walks the file character-by-character
    # (skipping string contents, honoring backslash escapes) and confirms
    # every {}/[] opens and closes and no string is left unterminated. This
    # is not a full JSON grammar check, but it reliably catches the failure
    # modes that actually occur here — truncation, a hand-edited fixture
    # with a dropped brace, an empty file — without needing a JSON library.
    awk '
        BEGIN { depth = 0; instr = 0; esc = 0; saw_token = 0 }
        {
            n = length($0)
            for (i = 1; i <= n; i++) {
                c = substr($0, i, 1)
                if (esc) { esc = 0; continue }
                if (instr) {
                    if (c == "\\") { esc = 1 }
                    else if (c == "\"") { instr = 0 }
                    continue
                }
                if (c == "\"") { instr = 1; saw_token = 1; continue }
                if (c == "{" || c == "[") { depth++; saw_token = 1; continue }
                if (c == "}" || c == "]") { depth--; if (depth < 0) exit 1 }
            }
        }
        END {
            if (instr) exit 1
            if (depth != 0) exit 1
            if (!saw_token) exit 1
            exit 0
        }
    ' "$1"
}

for f in command_v1.json event_v1.json; do
    path="$GOLDEN_DIR/$f"
    if [ -f "$path" ]; then
        if ! check_json_balanced "$path"; then
            echo "    [FAIL] $path is not well-formed JSON (unbalanced braces/brackets or unterminated string)"
            FAILED=1
        fi
    fi
done

if [ -f "$GOLDEN_DIR/sync_message_v1.bin" ]; then
    # Structurally parse the frozen SyncMessage layout (message.rs doc
    # comment, reproduced there):
    #   [1]  message_type      (must be 0-5 — SyncMessageType has exactly
    #                            6 variants; message.rs's from_u8 rejects
    #                            anything else with InvalidMessageType)
    #   [16] message_id
    #   [1]  version_len (N)  + [N] version
    #   [16] organization_id
    #   [16] sender_replica
    #   [1]  target_present   (must be 0 or 1)
    #   [16] target_replica
    #   [4]  payload_len (N, u32 BE) + [N] payload
    #   [8]  timestamp
    #   [1]  signature_present (must be 0 or 1)
    #   [2]  signature_len (N, u16 BE) + [N] signature
    # The previous version of this check only asserted the file was >= 18
    # bytes — covering just message_type + message_id and ignoring every
    # other required field. That is a false negative: 18 zero bytes (or
    # any 18-byte blob, including one with an out-of-range message_type
    # like 99, which from_u8 would reject) passed. The parse below instead
    # walks every field and requires the declared lengths to consume the
    # file EXACTLY — no shorter (truncated) and no longer (trailing
    # garbage) — and range-checks every field that has a closed range.
    bin_path="$GOLDEN_DIR/sync_message_v1.bin"
    size=$(wc -c < "$bin_path" | tr -d ' ')
    if [ "$size" -eq 0 ]; then
        echo "    [FAIL] $bin_path is empty"
        FAILED=1
    else
        bytes_line=$(od -An -v -tu1 "$bin_path" | tr -s ' \n' ' ')
        if ! awk -v line="$bytes_line" -v total="$size" '
            function byte(i) { return b[off + i] + 0 }
            function u16be(i) { return byte(i) * 256 + byte(i + 1) }
            function u32be(i) { return byte(i) * 16777216 + byte(i+1) * 65536 + byte(i+2) * 256 + byte(i+3) }
            BEGIN {
                n = split(line, b, " ")
                # split() may leave a leading empty field from the tr output
                off = 1
                if (b[1] == "") off = 2

                if (total < 1) exit 1
                message_type = byte(0)
                if (message_type < 0 || message_type > 5) exit 1
                p = 1
                p += 16                              # message_id
                if (p + 1 > total) exit 1
                version_len = byte(p)
                p += 1 + version_len                 # version_len byte + version bytes
                if (p + 16 + 16 + 1 > total) exit 1
                p += 16                              # organization_id
                p += 16                              # sender_replica
                target_present = byte(p)
                if (target_present != 0 && target_present != 1) exit 1
                p += 1
                p += 16                               # target_replica (always on wire)
                if (p + 4 > total) exit 1
                payload_len = u32be(p)
                p += 4 + payload_len
                if (p + 8 + 1 > total) exit 1
                p += 8                                # timestamp
                signature_present = byte(p)
                if (signature_present != 0 && signature_present != 1) exit 1
                p += 1
                if (p + 2 > total) exit 1
                signature_len = u16be(p)
                p += 2 + signature_len
                if (p != total) exit 1                # must consume the file exactly
                exit 0
            }
        '; then
            echo "    [FAIL] $bin_path does not structurally match the frozen SyncMessage layout"
            echo "           (message_type out of range 0-5, a length-prefixed field running past"
            echo "           end of file, or trailing bytes left over after the declared fields)"
            FAILED=1
        fi
    fi
fi

[ "$FAILED" -eq 0 ] && [ "$MISSING_FIXTURES" -eq 0 ] && echo "    ok"
[ "$MISSING_FIXTURES" -eq 1 ] && echo "    skipped (fixtures missing, see [2/3])"

echo ""
if [ $FAILED -eq 0 ]; then
    echo "[PASS] Wire format fixtures present and well-formed"
    exit 0
else
    echo "[FAIL] Golden fixtures for wire formats are missing or invalid."
    echo ""
    echo "       tests/golden/ must contain command_v1.json, event_v1.json and"
    echo "       sync_message_v1.bin, generated FROM THE REAL SERIALIZERS — never"
    echo "       hand-written. Per Team Prompt 1 §8.3's amendment procedure:"
    echo ""
    echo "         cargo test --test golden_fixtures -- --ignored"
    echo ""
    echo "       then copy the generated output into tests/golden/ and commit it."
    echo "       As of this run, no such test target exists in the repository —"
    echo "       it must be written (against CommandEnvelope, DomainEventEnvelope"
    echo "       and SyncMessage, using fixed, non-random sample values so the"
    echo "       output is byte-reproducible across runs) before this command will"
    echo "       do anything. Do not hand-author the fixture files to make this"
    echo "       check pass — that locks in a format nobody verified."
    exit 1
fi
