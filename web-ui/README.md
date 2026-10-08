# ONYX web-ui

React + Vite thin client for `api-server`. Talks real HTTP/WebSocket to
`api-server` today (unlike the mobile/desktop clients — see
`docs/AUDIT_REGISTER.md` and `docs/SESSION7_README.md` for that
distinction and why it matters).

## Local development (this machine only)

```bash
cp .env.example .env
npm install
npm run dev
```

Defaults (`.env.example`) point at `api-server` on `127.0.0.1:3000` —
correct if both `api-server` and this dev server run on the same machine
and you're only accessing the UI from that machine's own browser.

## Running across your local network (LAN)

To reach this UI, and have it reach `api-server`, from another device on
your LAN (phone, another PC, etc.), three separate binds need to change —
missing any one of them will look like it's "not working" even though the
other two are correct:

### 1. `api-server` must bind to all interfaces, not just localhost

Start it with `ONYX_BIND=0.0.0.0:3000` (see `start-backend.ps1 -Bind
0.0.0.0:3000` at the repo root, or set the env var directly). `127.0.0.1`
never accepts connections from other devices, regardless of any other
setting — this is the most common thing to miss.

### 2. This Vite dev server must also bind to all interfaces

Already configured in `vite.config.ts` (`server: { host: true }`). No
action needed — just run `npm run dev` as normal and Vite will print both
a `Local` and a `Network` URL; use the `Network` one from other devices.

### 3. Point this UI's API calls at your machine's LAN IP, not `127.0.0.1`

`127.0.0.1` inside a value like `VITE_API_BASE` means "the device the
*browser* is running on" — on another device, that's not this machine, so
`127.0.0.1` here is never correct for LAN access, even though `api-server`
is genuinely reachable elsewhere.

Find this machine's LAN IP:
- Windows: `ipconfig` → `IPv4 Address` under your active adapter
- macOS/Linux: `ifconfig` or `ip addr` → look for your Wi-Fi/Ethernet
  adapter's `inet` address (typically `192.168.x.x` or `10.x.x.x`)

Then set `.env` (not `.env.example` — that file is the template, `.env`
is what Vite actually reads and is gitignored):

```
VITE_API_BASE=http://192.168.1.50:3000
VITE_WS_BASE=ws://192.168.1.50:3000
```

(substitute your machine's actual LAN IP)

Restart `npm run dev` after changing `.env` — Vite only reads env files at
startup, not on every request.

### Firewall

Windows Firewall (or your OS equivalent) may prompt to allow incoming
connections the first time another device connects to ports `3000` (API)
or `5173` (Vite dev server). Allow both for your local network profile.
Corporate/managed machines may block this at a policy level outside your
control — if devices still can't connect after steps 1–3, that's the next
thing to check.

### Security note

`0.0.0.0` binds expose the service to your *entire* local network, not
just the specific device you're testing from. Fine for local development
on a trusted home/office network; do not do this on a public or
untrusted network (coffee shop Wi-Fi, etc.).
