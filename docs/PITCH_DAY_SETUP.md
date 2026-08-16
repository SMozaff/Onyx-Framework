# Running ONYX on Your Own Machine — Pitch Day Setup

This mirrors exactly what was verified working end-to-end in the audit sandbox.
Total setup time: ~10 minutes on a machine with internet access.

---

## Why your own computer, not a cloud sandbox

For a live pitch you want:
- **No network dependency** during the demo — SQLite mode needs nothing external.
- **No surprise disconnects** — a rented sandbox session can end mid-meeting.
- **Instant restart** if something goes sideways in front of the room.

SQLite mode (what we verified) gives you all three. Postgres/Supabase is for
later — don't add that variable on pitch day.

---

## 1. One-time setup (do this the night before, not in the room)

### Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
  --default-toolchain 1.97.1 --profile minimal --component clippy,rustfmt
source "$HOME/.cargo/env"
```

### Install Node.js 22+ (if not already present)
Check with `node --version`. If missing, use https://nodejs.org (LTS).

### Get the code
Unzip the delivered `Onyx_fixed.zip` (already has the C-01/C-02/H-01 fixes
applied — do **not** use the original archive, it doesn't compile).

### Build everything once
```bash
cd Onyx
export SQLX_OFFLINE=true
cargo build -p api-server --bin api-server --release
cd web-ui && npm install && npm run build && cd ..
```

`--release` matters for pitch day: it's a slower build now but a snappier demo.
This step takes 5–10 minutes depending on your machine — run it well before
the meeting, not while people are walking in.

> **Desktop app:** also needs GTK/WebKit installed (`libgtk-3-dev`,
> `libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev` on Linux; on macOS/Windows, Tauri's
> normal prerequisites apply — see https://tauri.app/start/prerequisites/).
> Confirm this the night before on the actual pitch machine.

---

## 2. On pitch morning — start the server

Save this as `start_pitch_demo.sh`:

```bash
#!/bin/bash
export DATABASE_URL="sqlite:$HOME/onyx_pitch_demo.db?mode=rwc"
export ONYX_ENV=development
export ONYX_BIND=127.0.0.1:3000
export ONYX_METRICS_BIND=127.0.0.1:9090
export ONYX_BOOTSTRAP_TOKEN="pitch-demo-bootstrap"
export ONYX_AUTHORITY_SIGNING_KEY="hex:$(python3 -c 'import secrets;print(secrets.token_hex(32))')"
exec ./target/release/api-server
```

```bash
chmod +x start_pitch_demo.sh
rm -f ~/onyx_pitch_demo.db      # fresh state each run
./start_pitch_demo.sh
```

Confirm it's alive: `curl http://127.0.0.1:3000/health` → `{"status":"ok"}`

**Create your demo user once, before anyone arrives:**
```bash
curl -X POST http://127.0.0.1:3000/api/admin/bootstrap \
  -H 'Content-Type: application/json' \
  -H 'x-onyx-bootstrap-token: pitch-demo-bootstrap' \
  -d '{"username":"demo","password":"demo-pitch-password-2026"}'
```
Do this **before** the meeting, not live — the response includes a JWT you
don't want on the projector.

---

## 3. Serve the web UI

```bash
cd web-ui && npm run preview -- --port 5173
```
Open `http://localhost:5173` in the browser you'll present from. Log in with
`demo` / `demo-pitch-password-2026`.

---

## 4. If you also want the desktop app running

```bash
cd crates/bins/desktop-shell
cargo tauri build --debug    # faster than a full release build
```
The binary lands under `target/debug/bundle/`. Launch it after the API server
is already up — the desktop shell talks to the same `127.0.0.1:3000`.

---

## 5. Fallback if live demo risk is too high

Run the same steps once beforehand, screen-record a 90-second walkthrough, and
have both the live app *and* the recording ready. If Wi-Fi or a laptop hiccup
happens mid-pitch, switch to the recording without breaking stride — nobody
in the room will know the difference.

---

## What NOT to do on pitch day

- Don't run `cargo build` (debug, unoptimized) live — first response will be
  noticeably slower and undercut the "production ready" story.
- Don't demo the bootstrap endpoint live — it's a one-time admin-creation flow,
  not a feature; showing it invites "wait, is auth actually done?" questions
  that pull focus from the product.
- Don't connect it to Supabase/any cloud DB for the demo unless you've tested
  that exact network from that exact room beforehand.
