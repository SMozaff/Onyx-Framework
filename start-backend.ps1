#Requires -Version 5.1
<#
.SYNOPSIS
    Checks prerequisites and starts the ONYX api-server backend locally.

.DESCRIPTION
    Mirrors the manual steps already verified working in this repo's own
    session history (see docs/AUDIT_REGISTER.md, docs/SESSION7_README.md):
      - Rust 1.97.1 (pinned in rust-toolchain.toml) via rustup
      - SQLX_OFFLINE=true, using the repo's committed .sqlx/ query cache
        (no live database connection needed at compile time)
      - No DATABASE_URL required for local dev: api-server defaults to a
        local SQLite file and runs its own sqlx::migrate! on startup
        (crates/bins/api-server/src/main.rs, routes/mod.rs) — this script
        does not run migration-tool separately, since it isn't needed.

    This script only CHECKS and STARTS the backend. It does not install
    Rust for you — see the printed instructions if rustup is missing,
    matching the two-step "check first, install if needed" pattern used
    earlier in this project for Flutter.

.PARAMETER Bind
    Address:port for the HTTP/WebSocket API. Default matches
    crates/bins/api-server/src/main.rs's own fallback: 127.0.0.1:3000.
    Pass 0.0.0.0:3000 instead if a physical device on your LAN needs to
    reach this server (127.0.0.1 only accepts connections from this PC).

.PARAMETER MetricsBind
    Address:port for the Prometheus metrics endpoint. Default matches
    main.rs's own fallback: 0.0.0.0:9090.

.PARAMETER DatabaseUrl
    Optional. If omitted, api-server falls back to its own default
    (sqlite://onyx-team7.db?mode=rwc) — a local file created on first run.

.EXAMPLE
    .\start-backend.ps1

.EXAMPLE
    .\start-backend.ps1 -Bind 0.0.0.0:3000
    Use this if testing from a physical phone on the same Wi-Fi network,
    then point the mobile app at http://<this-PC's-LAN-IP>:3000 instead
    of localhost — the phone cannot reach 127.0.0.1 on your PC.
#>

[CmdletBinding()]
param(
    [string]$Bind = "127.0.0.1:3000",
    [string]$MetricsBind = "0.0.0.0:9090",
    [string]$DatabaseUrl = ""
)

$ErrorActionPreference = "Stop"

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Ok {
    param([string]$Message)
    Write-Host "    [OK] $Message" -ForegroundColor Green
}

function Write-Fail {
    param([string]$Message)
    Write-Host "    [MISSING] $Message" -ForegroundColor Red
}

# Resolve repo root as the directory this script lives in, matching the
# project's own scripts/ci-pipeline.sh convention (ROOT = script's own
# directory, not the caller's current directory).
$RepoRoot = $PSScriptRoot
if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
}

Write-Step "Checking prerequisites"

# This script must live in the repo root (next to Cargo.toml) to work —
# it is not meant to be run from wherever it was downloaded to. Fail fast
# with a clear instruction rather than limping through with warnings and
# failing confusingly later at 'cargo run' with "could not find Cargo.toml".
$cargoTomlPath = Join-Path $RepoRoot "Cargo.toml"
if (-not (Test-Path $cargoTomlPath)) {
    Write-Fail "No Cargo.toml found next to this script (looked in: $RepoRoot)"
    Write-Host ""
    Write-Host "    This script must be copied into your ONYX repo root before running," -ForegroundColor Yellow
    Write-Host "    e.g.:" -ForegroundColor Yellow
    Write-Host "      Copy-Item `"`$env:USERPROFILE\Downloads\start-backend.ps1`" `"C:\Users\Lenovo\Documents\GitHub\Onyx-Framwork\`""
    Write-Host "      cd `"C:\Users\Lenovo\Documents\GitHub\Onyx-Framwork`""
    Write-Host "      .\start-backend.ps1"
    Write-Host ""
    exit 1
}

# --- Rust toolchain -----------------------------------------------------
$rustcFound = Get-Command rustc -ErrorAction SilentlyContinue
$cargoFound = Get-Command cargo -ErrorAction SilentlyContinue

if (-not $rustcFound -or -not $cargoFound) {
    Write-Fail "Rust toolchain (rustc/cargo) not found on PATH"
    Write-Host ""
    Write-Host "    Install it:" -ForegroundColor Yellow
    Write-Host "      1. Go to https://rustup.rs and download/run rustup-init.exe"
    Write-Host "      2. Choose the default installation (option 1)"
    Write-Host "      3. Close and reopen PowerShell"
    Write-Host "      4. Re-run this script"
    Write-Host ""
    exit 1
}

$rustcVersionOutput = & rustc --version
Write-Ok "Rust found: $rustcVersionOutput"

# Verify the pinned toolchain version from rust-toolchain.toml is actually
# installed. cargo will auto-select it via rustup's override mechanism as
# long as it's installed; this only checks presence, not active-ness, to
# avoid false negatives if rustup is managing this correctly already.
$toolchainFile = Join-Path $RepoRoot "rust-toolchain.toml"
if (Test-Path $toolchainFile) {
    $pinnedVersion = (Select-String -Path $toolchainFile -Pattern 'channel\s*=\s*"([^"]+)"').Matches.Groups[1].Value
    if ($pinnedVersion) {
        $rustupFound = Get-Command rustup -ErrorAction SilentlyContinue
        if ($rustupFound) {
            $installedToolchains = & rustup toolchain list 2>$null
            if ($installedToolchains -match [regex]::Escape($pinnedVersion)) {
                Write-Ok "Pinned toolchain $pinnedVersion is installed"
            } else {
                Write-Host "    [WARN] Pinned toolchain $pinnedVersion not found in 'rustup toolchain list'." -ForegroundColor Yellow
                Write-Host "           Installing it now (cargo/rustup will otherwise fail before build)..." -ForegroundColor Yellow
                & rustup toolchain install $pinnedVersion --profile minimal --no-self-update
                if ($LASTEXITCODE -ne 0) {
                    Write-Fail "Failed to install pinned toolchain $pinnedVersion"
                    exit 1
                }
                Write-Ok "Installed toolchain $pinnedVersion"
            }
        } else {
            Write-Host "    [WARN] rustup not found; cannot verify pinned toolchain $pinnedVersion is active. Continuing anyway." -ForegroundColor Yellow
        }
    }
} else {
    Write-Host "    [WARN] rust-toolchain.toml not found at $toolchainFile — are you running this from the repo?" -ForegroundColor Yellow
}

# --- .sqlx cache ----------------------------------------------------------
$sqlxCacheDir = Join-Path $RepoRoot ".sqlx"
if (Test-Path $sqlxCacheDir) {
    Write-Ok ".sqlx/ query cache found (needed for SQLX_OFFLINE=true build)"
} else {
    Write-Host "    [WARN] .sqlx/ not found at $sqlxCacheDir." -ForegroundColor Yellow
    Write-Host "           The build may fail without a live DATABASE_URL at compile time." -ForegroundColor Yellow
}

Write-Step "Configuring environment"

$env:SQLX_OFFLINE = "true"
Write-Ok "SQLX_OFFLINE=true"

if ($DatabaseUrl -ne "") {
    $env:DATABASE_URL = $DatabaseUrl
    Write-Ok "DATABASE_URL=$DatabaseUrl"
} else {
    # Deliberately not setting DATABASE_URL: api-server's own main.rs falls
    # back to sqlite://onyx-team7.db?mode=rwc and runs sqlx::migrate!
    # itself on startup (see routes/mod.rs) — no separate migration-tool
    # step is needed for local dev.
    Write-Ok "DATABASE_URL not set — api-server will use its own local SQLite default"
}

$env:ONYX_BIND = $Bind
Write-Ok "ONYX_BIND=$Bind"

$env:ONYX_METRICS_BIND = $MetricsBind
Write-Ok "ONYX_METRICS_BIND=$MetricsBind"

if ($Bind -like "0.0.0.0:*") {
    Write-Host ""
    Write-Host "    NOTE: binding to 0.0.0.0 exposes this server to your whole LAN," -ForegroundColor Yellow
    Write-Host "    not just this PC. Fine for local dev on a trusted network; do not" -ForegroundColor Yellow
    Write-Host "    do this on a public/untrusted network." -ForegroundColor Yellow
}

Write-Step "Building and starting api-server (first build will take several minutes)"

Push-Location $RepoRoot
try {
    cargo run -p api-server
}
finally {
    Pop-Location
}
