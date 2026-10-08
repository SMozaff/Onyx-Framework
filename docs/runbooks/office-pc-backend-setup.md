# Running `api-server` on a dedicated office Linux PC (UX testing)

Purpose: a standalone backend for testing/debugging and evolving the UX —
not a production deployment. Written 2026-08-15.

## What this PC needs to run

Only `api-server` — the GUI apps (`desktop-shell`, `admin-shell`, `web-ui`)
are what run on *your* machine and talk to this box, not the other way
around. You do not need to build the Tauri apps on this PC at all, which
skips the heavy GTK/WebKit2GTK dependency chain entirely.

## 1. Install the toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

The repo pins its exact compiler version in `rust-toolchain.toml`
(`channel = "1.97.1"`) — `rustup` reads this automatically and installs
the matching version the first time you build inside the repo. You don't
need to select a version manually.

## 2. System dependencies

None required for `api-server` specifically:
- TLS is handled by `rustls` (pure Rust) — no system OpenSSL needed.
- SQLite is statically bundled via `sqlx`'s `sqlite` feature — no system
  SQLite package needed.
- Postgres is supported too (`sqlx`'s `postgres` feature talks the wire
  protocol directly), but isn't required — SQLite is the default and is
  enough for this purpose.

If `cargo build` still complains about a linker or `pkg-config`, that's
almost always just `build-essential` missing:
```bash
sudo apt update && sudo apt install -y build-essential pkg-config
```

## 3. Get the code onto the box

Either `git clone` the real repo, or transfer the tarball already
produced this session (`Onyx-Framwork-updated.tar.gz`, excludes
`target/`/`node_modules/`) and extract it.

## 4. Build

```bash
cd Onyx-Framwork-main
cargo build --release --package api-server
```
`--release` is worth it here even for testing — debug builds of a server
you'll be hitting repeatedly from a UI are noticeably slower. First build
will take a while (fresh dependency compile); subsequent builds after
code changes are incremental and fast.

## 5. Run it

```bash
ONYX_BIND=0.0.0.0:3000 \
DATABASE_URL="sqlite:///home/<you>/onyx-data/onyx.db?mode=rwc" \
./target/release/api-server
```

- `ONYX_BIND=0.0.0.0:3000` — binds to all network interfaces, not just
  localhost, so other machines on the office LAN can reach it. Defaults
  to `127.0.0.1:3000` (localhost-only) if unset — fine only if everything
  testing against it runs on this same PC.
- `DATABASE_URL` — defaults to `sqlite://onyx-team7.db?mode=rwc` in the
  current directory if unset. Pointing it at an explicit path keeps your
  test data in one predictable place you can back up or wipe between
  test rounds (`rm onyx.db` to reset to empty).

## 6. Reach it from another machine

Find this PC's LAN IP:
```bash
ip addr show | grep "inet "
```
Then point `desktop-shell`/`admin-shell`/`web-ui` at
`http://<that-ip>:3000` instead of `localhost:3000`, from whatever
machine is doing the UX testing.

## 7. Bootstrap the first user

`api-server` starts with no users. Set a bootstrap token before first
run and use it once to create the first Admin account — see
`docs/runbooks/user-class-migration.md` for the exact `curl` sequence,
or ask me to walk through it when you're ready to do this on the box.

## Explicitly out of scope for this setup
- No reverse proxy / TLS termination — this is LAN-only, plain HTTP, by
  design, since it's for internal testing.
- No process supervisor (systemd unit, etc.) — run it in a terminal or
  `tmux`/`screen` session for now; worth adding once this becomes more
  than ad hoc testing.
- No Postgres setup — SQLite is enough for this purpose; revisit only if
  you specifically want to test Postgres-specific behavior.
