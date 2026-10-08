# ONYX Server Installation & Client Connection Guide

Covers both supported server platforms — **Windows 10 / Windows Server** and **Debian 13** — plus how the **Admin** and **Staff** client apps connect to whichever server you stand up. One document, because the process and the failure modes largely mirror each other across both OSes.

This guide describes the automated setup scripts (`scripts/setup-onyx-windows.ps1`, `scripts/setup-onyx-debian.sh`), which are the supported way to install the server. They install a **prebuilt** `api-server` binary you already have — they do not compile from source or download the binary for you.

---

## 1. Before you start

You need:

- A trusted, prebuilt `api-server` executable (Windows: `api-server.exe`, Linux: `api-server`) — obtained from a release build, not built by this script.
- Administrator access on Windows, or `sudo`/root on Debian.
- The machine's role decided: is this a permanent server, or a temporary one (e.g. someone's desk PC)? The script works the same either way, but a temporary machine won't reliably stay reachable — see §6.

You do **not** need Rust, Node, or any build tooling installed on the server machine. The binary is prebuilt.

---

## 2. Installing the server

### Windows

Open **PowerShell as Administrator**, navigate to the folder containing the script, and run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\setup-onyx-windows.ps1 `
  -ApiServerPath C:\Users\Administrator\Downloads\api-server.exe `
  -AdminUsername admin
```

`-ExecutionPolicy Bypass` here only affects this one PowerShell process — it does not change your PC's execution policy permanently, and it stops applying the moment this window closes.

You'll be prompted for the new Admin's password (not echoed to the screen, not logged anywhere).

**What this does, in order:**
1. Copies your binary into `C:\ProgramData\ONYX`.
2. Creates a fresh SQLite database there.
3. Binds the server to `0.0.0.0:3000` — reachable from other machines on the network, not just this PC.
4. Creates a named Windows Firewall rule (`ONYX-API-HTTP`), scoped by default to **Private/Domain** networks and the **local subnet only**.
5. Starts the server as a normal background process.
6. Creates the first Admin account.
7. Removes the one-time bootstrap token.
8. **Proves it actually works**: polls `/health`, then does a real login with the account it just created.

If every step succeeds, you'll see a completion message with the address other machines should use.

#### Optional: run as a Windows service (survives reboot/logoff)

By default the server stops if you log off or restart the PC. To make it persistent, obtain [NSSM](https://nssm.cc/) separately (the script does not download it for you — a deliberate choice, since silently fetching and running a third-party service manager is a real supply-chain risk) and add:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\setup-onyx-windows.ps1 `
  -ApiServerPath C:\Users\Administrator\Downloads\api-server.exe `
  -AdminUsername admin `
  -InstallService `
  -NssmPath C:\Tools\nssm\win64\nssm.exe
```

The service runs as the low-privilege `NT AUTHORITY\LocalService` account, with write access granted only to ONYX's own data and log folders — not the whole system.

### Debian 13

Transfer your trusted `api-server` binary to the machine, then:

```bash
chmod 0755 scripts/setup-onyx-debian.sh
sudo ./scripts/setup-onyx-debian.sh \
  --api-server /home/admin/api-server \
  --admin-username admin
```

You'll be prompted for the Admin password if you don't pass `--admin-password`.

**What this does, in order:**
1. Installs its own minimal prerequisites: `ca-certificates`, `curl`, `jq`, `openssl` (utilities the script itself needs for checks and token generation — ONYX's own TLS stack uses `rustls`, not OpenSSL, so this isn't a server runtime dependency).
2. Creates an unprivileged `onyx` system account.
3. Installs the binary to `/opt/onyx`, database to `/var/lib/onyx`, logs to `/var/log/onyx`.
4. Binds to `0.0.0.0:3000`.
5. If **UFW is installed and active**, opens the port — but only from private IPv4 ranges by default. If UFW isn't installed or isn't active, the script changes nothing (it doesn't install or enable UFW for you).
6. Starts the server, creates the first Admin, removes the bootstrap token, verifies with a real `/health` + login check — same as Windows.

#### Optional: run as a systemd service (survives reboot/logout)

```bash
sudo ./scripts/setup-onyx-debian.sh \
  --api-server /home/admin/api-server \
  --admin-username admin \
  --install-service
```

Runs as the unprivileged `onyx` user, restarts automatically on failure, starts at boot. The generated unit is hardened (`NoNewPrivileges`, `ProtectSystem=strict`, write access limited to the database/log paths).

---

## 3. Finding the server's address

Both scripts print the address to use at the end. If you need it again later:

- **Windows:** `ipconfig` → look for "IPv4 Address" (usually `192.168.x.x` or `10.x.x.x`).
- **Linux:** `ip addr show` or `hostname -I`.

The server itself can always reach itself at `http://127.0.0.1:3000`. Every other machine must use the LAN IP, e.g. `http://192.168.0.250:3000` — `127.0.0.1` means "this same computer" to whichever device is trying to connect, so typing it into a client on a *different* PC will never work.

---

## 4. Connecting the client apps

Neither client app needs a rebuild to point at your server — both have a real, saveable server-address setting.

### Admin app

On the login screen, tap **"Server address / connection settings"** (collapsed by default). Enter `http://<server-LAN-IP>:3000`, press **Test & Save**. It actually checks reachability before saving — an unreachable address is never silently stored. Then log in with the Admin username/password from setup.

If you're already logged in and need to point at a different server, the same setting is available from the Settings page.

### Staff app (native desktop)

The server address is entered right on the login form, alongside username and password — this app ties its session to a specific server, so there's no separate address setting to configure before logging in. Enter the address, sign in.

To switch servers later: open Settings, enter the new address, test it — a successful test signs you out and returns you to the login screen rather than silently swapping servers under an existing session. This is deliberate: a login token from one server is meaningless (and a real security risk) if reused against a different one.

---

## 5. Verifying it all actually works end to end

A clean run of either setup script is already strong evidence — it performed a real login, not just a health check. But if you want to double-check manually:

```
curl http://<server-LAN-IP>:3000/health
```
should return a JSON body indicating the server is healthy. Then try logging into the Admin or Staff app from a *different* machine on the network using that same address.

---

## 6. Probable issues and their solutions

### "The script demands Administrator/root, but I'm not sure I ran it that way"

**Windows** — you'll see: *"Run this script from an elevated Windows PowerShell session."* Right-click PowerShell → "Run as Administrator," then re-run the exact command.

**Debian** — you'll see: *"Run this script with sudo/root privileges."* Prefix the command with `sudo`.

### "Port 3000 is already listening" / setup refuses to start

Both scripts check for an existing listener on the bind port before doing anything, and refuse to proceed rather than risk a silent conflict.

- **Windows** tells you the exact PID(s) holding the port. Check what it is (`Get-Process -Id <pid>`) before deciding to stop it — it may be a previous ONYX run you forgot was still open.
- **Debian**: same idea — `ss -ltnp | grep :3000` to find the process, stop it, re-run.

Likely cause: you ran the script once before, closed the terminal without stopping the server cleanly (Ctrl+C, not the window's X button), and the process is still alive in the background.

### "Database already exists" — the script won't reinstall

Both scripts treat an existing database as protected data and refuse to overwrite it. This is intentional — a script that silently wiped a real database on a second run would be far more dangerous than one that stops and asks.

- If you genuinely want a fresh install: back up or delete the old database file first (`onyx.db` under the install root), then re-run.
- If you want to **keep** the existing data and just confirm a known Admin account still works: re-run with `-ReuseExistingDatabase` (Windows) / `--reuse-existing-database` (Debian). This skips creating a new account and instead does a real login check against the account you specify.

### The server started, but nothing on the network can reach it

Work through these in order — each one is a distinct thing the setup script already tries to prevent, but any of them can still go wrong on a specific network:

1. **Wrong address on the client.** `127.0.0.1` only ever means "the device you typed it into." Confirm you're using the server's actual LAN IP (§3), not localhost, on every other machine.
2. **Client and server aren't actually on the same network.** A laptop on a phone hotspot or a different Wi-Fi network won't see a LAN IP from another network. Confirm both devices show IPs in the same subnet range.
3. **Firewall blocked it anyway.** The scripts create a rule, but a stricter *corporate* firewall/group policy, a *router*-level firewall, or a security suite (e.g. third-party antivirus with its own firewall) can still block the port independently of what the script configured. Test locally first (`curl http://127.0.0.1:3000/health` on the server itself) to isolate whether the server is even up, before chasing network config.
4. **The server's IP address changed.** Home/office routers often hand out IPs via DHCP, which can change after a router restart, unless you've set a static/reserved IP for the server machine. If clients that worked yesterday suddenly can't connect, check whether the address actually changed before assuming something's broken.

### "The API did not become healthy within 30 seconds"

The server process started but never reported itself ready in time. Both scripts tell you exactly where to look:
- Windows: check the stdout/stderr logs under `C:\ProgramData\ONYX\logs\`.
- Debian: check `/var/log/onyx/api-server.log`.

Common causes worth checking in the log: a corrupted/incompatible existing database file (see the "Database already exists" case above — if you bypassed that check by hand-deleting files inconsistently, the server may fail differently than a clean fresh install), or the bind port being silently reserved by something else the earlier port-check didn't catch (rare, but possible with certain VPN/virtualization network adapters).

### "First-admin bootstrap failed" or "Login verification failed"

These come from the two real verification calls at the end of setup. If the server is healthy but either of these fails, it means the *account creation or login logic itself* rejected the request — not a network problem. This is a genuine code-level issue if it happens on a normal fresh install; it shouldn't occur under standard use. If you hit this, capture the exact error text the script prints and the corresponding server log entry before troubleshooting further, since guessing at the cause without both pieces of evidence tends to waste time chasing the wrong fix.

### The Windows service won't start, or `-InstallService` fails

- *"-InstallService requires -NssmPath pointing to a trusted nssm.exe"* — you passed `-InstallService` without a valid path to an actual `nssm.exe` file. Download NSSM yourself first, verify it, then point `-NssmPath` at it.
- *"Service ... did not reach Running state"* — NSSM installed the service, but Windows couldn't actually start it. Check the Windows Event Viewer (Application log) and the same stdout/stderr log files as above — most often this is a permissions issue on the data/log folders, since the service runs as `LocalService`, not your own elevated account.

### The Debian systemd service won't start

*"systemd did not report onyx-api.service as active"* — run `systemctl status onyx-api.service` and `journalctl -u onyx-api.service -n 50` immediately after; the script deliberately fails loudly here rather than reporting success on an unverified guess. Common cause: the `onyx` user lacking write access to a data/log path that was manually moved or reconfigured outside the script's own directory choices.

### Client app says "Invalid username or password" but you're sure the password is right

Both client apps intentionally show this same generic message for **both** a genuinely wrong password *and* an unreachable/misconfigured server address, when the failure happens quickly — this is a deliberate security choice (not confirming whether a username exists), but it means the message alone doesn't tell you which one it is.

- If the app shows a distinct "could not reach the server" message and auto-expands the server-address field, that's the app correctly identifying a network problem, not a credentials problem — fix the address, not the password.
- If it just says invalid credentials with no network-specific wording, the server was reached and genuinely rejected the login — recheck the password, or confirm you're using the right server if more than one ONYX instance exists on your network.

### You ran setup successfully once, but need to point a client at a *different* server later

This isn't an error case, just a reminder: use each app's server-settings screen (§4) rather than re-running the install script. The install scripts are for standing up a server, not for reconfiguring where a client looks.

---

## 7. What these scripts deliberately do *not* do

Documented here so it's not mistaken for an oversight:

- No source build, no binary download — you must supply a trusted binary yourself.
- No TLS/HTTPS configuration — this sets up plain HTTP on your local network. If you need this reachable outside your LAN, that requires additional, separate work (reverse proxy, real certificates) not covered here.
- No multi-server/high-availability setup — this is a single-server SQLite deployment.
- No automatic reinstall over existing data — protecting your database is treated as more important than convenience.
- No public-internet-facing firewall opening by default — you must explicitly opt in (`-AllowPublicNetwork` / `--allow-public-network`) and are expected to have separately decided that's actually safe for your situation.
