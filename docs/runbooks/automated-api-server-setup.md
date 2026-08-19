# Automated ONYX API Server Setup

The two scripts in `scripts/` install a **trusted, prebuilt** `api-server` binary for a single-server SQLite deployment, bind the API for desktop clients, create the first Admin only on a fresh database, and then perform two real checks: `GET /health` and `POST /api/auth/login`. They do not build Rust from source, download an executable, expose the metrics port, configure TLS, or make a multi-instance deployment safe.

| Platform | Script | Default API listener | Default persistence behavior |
| --- | --- | --- | --- |
| Windows 10 / Windows Server | `scripts/setup-onyx-windows.ps1` | `0.0.0.0:3000` | Regular background process; service installation is opt-in. |
| Debian 13 | `scripts/setup-onyx-debian.sh` | `0.0.0.0:3000` | Regular background process; systemd installation is opt-in. |

> Both scripts treat an existing database as protected state. They stop rather than overwrite it. Use the explicit reuse flag only when the named existing Admin account and password are known, because reuse performs a login verification instead of creating another user.

## Windows

Open **Windows PowerShell as Administrator**, review the script and the release binary, then run the following command. The `-ExecutionPolicy Bypass` option is process-scoped for this invocation; it does not change the device-wide policy. Microsoft documents that the Process scope disappears when the PowerShell session closes.[1]

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\setup-onyx-windows.ps1 `
  -ApiServerPath C:\Users\Administrator\Downloads\api-server.exe `
  -AdminUsername admin
```

The script requests the first Admin password without echoing it, generates a one-time bootstrap token if none was supplied, copies the binary into `C:\ProgramData\ONYX`, creates the SQLite database under that directory, and creates the named firewall rule `ONYX-API-HTTP`. By default, the rule is limited to **Domain** and **Private** profiles and `LocalSubnet`; the explicit `-AllowPublicNetwork` switch broadens it to any remote address. `New-NetFirewallRule` supports program, protocol, local-port, profile, and remote-address criteria, which is why the rule is both named and verified after creation.[2]

To make the API survive user logoff and reboot, obtain and validate `nssm.exe` separately, then pass the opt-in service parameters below. The script deliberately does not download a service manager. The service runs as `NT AUTHORITY\LocalService`, has automatic start configured, and receives write access only to ONYX’s database and log directories.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\setup-onyx-windows.ps1 `
  -ApiServerPath C:\Users\Administrator\Downloads\api-server.exe `
  -AdminUsername admin `
  -InstallService `
  -NssmPath C:\Tools\nssm\win64\nssm.exe
```

## Debian 13

Transfer a trusted Linux `api-server` executable to the machine, make the setup script executable, review it, then run it with `sudo`.

```bash
chmod 0755 scripts/setup-onyx-debian.sh
sudo ./scripts/setup-onyx-debian.sh \
  --api-server /home/admin/api-server \
  --admin-username admin
```

The script installs only the packages it itself needs for safe setup and verification: `ca-certificates`, `curl`, `jq`, and `openssl`. The ONYX runtime image itself uses `ca-certificates`, `curl`, and `tini`; the API’s Rust TLS stack does not require OpenSSL as a runtime dependency.[3] The script creates an unprivileged `onyx` account, stores the binary under `/opt/onyx`, stores the SQLite database under `/var/lib/onyx`, and logs under `/var/log/onyx`.

If UFW is installed **and active**, the script opens TCP/3000 only from RFC 1918 private IPv4 ranges. It leaves an absent or inactive UFW installation unchanged. `--allow-public-network` deliberately broadens the UFW rule to any address and should be used only when the deployment has separately chosen the appropriate internet-facing protections.

Use `--install-service` when systemd should manage automatic start and recovery. The generated service runs as the unprivileged `onyx` user, uses `Restart=on-failure`, reads a root-owned environment file, and is restricted to its database and logs as writable locations.

```bash
sudo ./scripts/setup-onyx-debian.sh \
  --api-server /home/admin/api-server \
  --admin-username admin \
  --install-service
```

## Verification and recovery

Both scripts poll `http://127.0.0.1:3000/health` until the API returns an OK status, create the first Admin via `POST /api/admin/bootstrap` only on a fresh database, remove the bootstrap token from the long-running process, then submit a real login request with the selected Admin credentials. A successful completion message is therefore evidence of server reachability, database migration/startup, first-user creation where applicable, and credential verification—not merely that a process was launched.

The default API listener is reachable from the server itself at `http://127.0.0.1:3000`. Other trusted LAN devices should point their ONYX clients at `http://<server-LAN-IP>:3000`.

## References

[1]: https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.security/set-executionpolicy?view=powershell-7.6 "Microsoft Learn: Set-ExecutionPolicy"
[2]: https://learn.microsoft.com/en-us/powershell/module/netsecurity/new-netfirewallrule?view=windowsserver2025-ps "Microsoft Learn: New-NetFirewallRule"
[3]: ../../deploy/docker/api-server.Dockerfile "ONYX API server container runtime dependencies"
