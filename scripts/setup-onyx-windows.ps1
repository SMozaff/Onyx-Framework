#Requires -Version 5.1
<#
.SYNOPSIS
    Installs and verifies a prebuilt ONYX api-server on Windows 10/Windows Server.

.DESCRIPTION
    This script intentionally changes the PowerShell execution policy only for
    its own process and creates a named Windows Firewall rule for the ONYX API.
    It copies a supplied, trusted api-server.exe into C:\ProgramData\ONYX,
    starts it, creates the first Admin on a fresh database, and verifies both
    /health and a real /api/auth/login response.

    Run an elevated Windows PowerShell session with this command (the Process
    execution-policy override ends when that PowerShell process exits):

      powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\setup-onyx-windows.ps1 `
        -ApiServerPath C:\Users\Administrator\Downloads\api-server.exe

    `-InstallService` is deliberately opt-in. It requires an already-obtained
    trusted copy of NSSM; the script never downloads or silently installs a
    service manager. NSSM starts the API as the low-privilege LocalService
    account and configures automatic restart, so it will continue through
    logoff and reboots.

    The default firewall rule is limited to Private/Domain profiles and the
    local subnet. Use -AllowPublicNetwork only after explicitly deciding that
    internet-facing exposure is acceptable and protecting the server separately.
#>

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateScript({ Test-Path -LiteralPath $_ -PathType Leaf })]
    [string]$ApiServerPath,

    [string]$InstallRoot = "C:\ProgramData\ONYX",

    [ValidatePattern('^[^:]+:\d+$')]
    [string]$Bind = "0.0.0.0:3000",

    [string]$MetricsBind = "127.0.0.1:9090",

    [string]$DatabasePath = "",

    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string]$AdminUsername = "admin",

    [SecureString]$AdminPassword,

    [string]$BootstrapToken = "",

    [switch]$ReuseExistingDatabase,

    [switch]$InstallService,

    [string]$NssmPath = "",

    [switch]$AllowPublicNetwork
)

Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass -Force
$ErrorActionPreference = "Stop"

$ServiceName = "ONYX-API"
$FirewallRuleName = "ONYX-API-HTTP"
$InstalledApiPath = Join-Path $InstallRoot "bin\api-server.exe"
$DataDirectory = Join-Path $InstallRoot "data"
$LogDirectory = Join-Path $InstallRoot "logs"
$StdoutLog = Join-Path $LogDirectory "api-server.stdout.log"
$StderrLog = Join-Path $LogDirectory "api-server.stderr.log"
$DatabaseDirectory = ""

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Ok {
    param([string]$Message)
    Write-Host "    [OK] $Message" -ForegroundColor Green
}

function Stop-WithError {
    param([string]$Message)
    Write-Host "    [FAILED] $Message" -ForegroundColor Red
    exit 1
}

function Assert-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        Stop-WithError "Run this script from an elevated Windows PowerShell session. See the command in its header."
    }
}

function Get-PortFromBind {
    param([string]$Address)
    return [int]($Address.Split(':')[-1])
}

function Convert-SecureStringToPlainText {
    param([SecureString]$Value)
    $pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($Value)
    try {
        return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    }
    finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    }
}

function New-RandomToken {
    $bytes = New-Object byte[] 32
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    return [Convert]::ToBase64String($bytes).Replace('+', '-').Replace('/', '_').TrimEnd('=')
}

function Wait-ForHealthyServer {
    param([string]$BaseUrl)
    for ($attempt = 1; $attempt -le 30; $attempt++) {
        try {
            $response = Invoke-WebRequest -Uri "$BaseUrl/health" -UseBasicParsing -TimeoutSec 2
            if ($response.StatusCode -eq 200) {
                Write-Ok "Health check passed at $BaseUrl/health"
                return
            }
        }
        catch {
            Start-Sleep -Seconds 1
        }
    }
    Stop-WithError "The API did not become healthy within 30 seconds. Check $StdoutLog and $StderrLog."
}

function Invoke-AdminBootstrap {
    param(
        [string]$BaseUrl,
        [string]$Username,
        [string]$Password,
        [string]$Token
    )
    $payload = @{ username = $Username; password = $Password; is_admin = $true } | ConvertTo-Json -Compress
    try {
        $created = Invoke-RestMethod -Uri "$BaseUrl/api/admin/bootstrap" -Method Post `
            -ContentType "application/json" -Headers @{ "x-onyx-bootstrap-token" = $Token } -Body $payload
    }
    catch {
        Stop-WithError "First-admin bootstrap failed: $($_.Exception.Message)"
    }
    if ($created.username -ne $Username -or -not $created.is_admin) {
        Stop-WithError "Bootstrap returned an unexpected user record."
    }
    Write-Ok "Created first Admin account '$Username'"
}

function Confirm-Login {
    param([string]$BaseUrl, [string]$Username, [string]$Password)
    $payload = @{ username = $Username; password = $Password } | ConvertTo-Json -Compress
    try {
        $login = Invoke-RestMethod -Uri "$BaseUrl/api/auth/login" -Method Post -ContentType "application/json" -Body $payload
    }
    catch {
        Stop-WithError "Login verification failed: $($_.Exception.Message)"
    }
    if ([string]::IsNullOrWhiteSpace($login.access_token) -or $login.user.username -ne $Username) {
        Stop-WithError "Login endpoint returned an incomplete or mismatched response."
    }
    Write-Ok "Real login verification passed for '$Username'"
}

function Set-ServiceEnvironment {
    param([string]$Token)
    $environment = @(
        "ONYX_BIND=$Bind",
        "ONYX_METRICS_BIND=$MetricsBind",
        "DATABASE_URL=$script:DatabaseUrl",
        "ONYX_ENV=development"
    )
    if (-not [string]::IsNullOrWhiteSpace($Token)) {
        $environment += "ONYX_BOOTSTRAP_TOKEN=$Token"
    }
    & $NssmPath set $ServiceName AppEnvironmentExtra $environment
    if ($LASTEXITCODE -ne 0) {
        Stop-WithError "NSSM could not configure the $ServiceName environment."
    }
}

function Start-DirectProcess {
    param([string]$Token)
    $env:ONYX_BIND = $Bind
    $env:ONYX_METRICS_BIND = $MetricsBind
    $env:DATABASE_URL = $script:DatabaseUrl
    $env:ONYX_ENV = "development"
    if ([string]::IsNullOrWhiteSpace($Token)) {
        Remove-Item Env:ONYX_BOOTSTRAP_TOKEN -ErrorAction SilentlyContinue
    }
    else {
        $env:ONYX_BOOTSTRAP_TOKEN = $Token
    }

    $process = Start-Process -FilePath $InstalledApiPath -WorkingDirectory $InstallRoot `
        -RedirectStandardOutput $StdoutLog -RedirectStandardError $StderrLog -PassThru
    Start-Sleep -Milliseconds 300
    if ($process.HasExited) {
        Stop-WithError "api-server exited immediately with code $($process.ExitCode). Check $StdoutLog and $StderrLog."
    }
    return $process
}

Assert-Administrator

Write-Step "Checking inputs and choosing paths"
$Port = Get-PortFromBind $Bind
if ($Port -lt 1 -or $Port -gt 65535) {
    Stop-WithError "Bind port must be between 1 and 65535."
}
if ([string]::IsNullOrWhiteSpace($DatabasePath)) {
    $DatabasePath = Join-Path $DataDirectory "onyx.db"
}
$DatabaseDirectory = Split-Path -Parent $DatabasePath
if ((Test-Path -LiteralPath $DatabasePath) -and -not $ReuseExistingDatabase) {
    Stop-WithError "Database already exists at $DatabasePath. Re-run with -ReuseExistingDatabase to preserve it and verify an existing Admin login."
}
if ($ReuseExistingDatabase -and -not (Test-Path -LiteralPath $DatabasePath)) {
    Stop-WithError "-ReuseExistingDatabase was supplied but no database exists at $DatabasePath."
}
if (-not $ReuseExistingDatabase -and [string]::IsNullOrWhiteSpace($BootstrapToken)) {
    $BootstrapToken = New-RandomToken
}
if ($null -eq $AdminPassword) {
    $AdminPassword = Read-Host "Password for '$AdminUsername' (not echoed or logged)" -AsSecureString
}
$PlainAdminPassword = Convert-SecureStringToPlainText $AdminPassword
if ([string]::IsNullOrWhiteSpace($PlainAdminPassword)) {
    Stop-WithError "An Admin password is required."
}
$existingListener = Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue
if ($existingListener) {
    Stop-WithError "Port $Port is already listening (PID(s): $($existingListener.OwningProcess -join ', ')). Stop the conflicting process first."
}
Write-Ok "Using API bind $Bind and SQLite database $DatabasePath"

Write-Step "Installing the trusted API binary and data directories"
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $InstalledApiPath), $DatabaseDirectory, $LogDirectory | Out-Null
Copy-Item -LiteralPath $ApiServerPath -Destination $InstalledApiPath -Force
# The optional NSSM service runs as LocalService, so grant only the data and
# log directories the write access it needs. The binary remains administrator-controlled.
icacls $DatabaseDirectory /grant "LOCAL SERVICE:(OI)(CI)M" /T /C | Out-Null
icacls $LogDirectory /grant "LOCAL SERVICE:(OI)(CI)M" /T /C | Out-Null
$DatabaseUrl = "sqlite:///" + $DatabasePath.Replace('\', '/') + "?mode=rwc"
Write-Ok "Installed api-server.exe at $InstalledApiPath"

Write-Step "Creating the Windows Firewall rule"
Remove-NetFirewallRule -Name $FirewallRuleName -ErrorAction SilentlyContinue
$firewallProfiles = if ($AllowPublicNetwork) { "Any" } else { @("Domain", "Private") }
$remoteAddress = if ($AllowPublicNetwork) { "Any" } else { "LocalSubnet" }
New-NetFirewallRule -Name $FirewallRuleName -DisplayName "ONYX API HTTP ($Port)" `
    -Description "Allows the ONYX api-server inbound TCP listener." -Enabled True `
    -Direction Inbound -Action Allow -Protocol TCP -LocalPort $Port -Program $InstalledApiPath `
    -Profile $firewallProfiles -RemoteAddress $remoteAddress | Out-Null
$rule = Get-NetFirewallRule -Name $FirewallRuleName -ErrorAction Stop
$portFilter = $rule | Get-NetFirewallPortFilter
if ($portFilter.LocalPort -notcontains "$Port" -or $rule.Enabled -ne "True") {
    Stop-WithError "Firewall rule verification did not find an enabled TCP/$Port rule."
}
Write-Ok "Enabled firewall rule '$FirewallRuleName' for TCP/$Port"

$serverUrl = "http://127.0.0.1:$Port"
$apiProcess = $null
if ($InstallService) {
    Write-Step "Configuring the opt-in Windows service"
    if ([string]::IsNullOrWhiteSpace($NssmPath) -or -not (Test-Path -LiteralPath $NssmPath -PathType Leaf)) {
        Stop-WithError "-InstallService requires -NssmPath pointing to a trusted nssm.exe. The script does not download service software."
    }
    $existingService = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($existingService) {
        & $NssmPath stop $ServiceName confirm | Out-Null
        & $NssmPath remove $ServiceName confirm | Out-Null
    }
    & $NssmPath install $ServiceName $InstalledApiPath
    if ($LASTEXITCODE -ne 0) { Stop-WithError "NSSM could not install $ServiceName." }
    & $NssmPath set $ServiceName AppDirectory $InstallRoot
    & $NssmPath set $ServiceName AppStdout $StdoutLog
    & $NssmPath set $ServiceName AppStderr $StderrLog
    & $NssmPath set $ServiceName AppRotateFiles 1
    & $NssmPath set $ServiceName Start SERVICE_AUTO_START
    & $NssmPath set $ServiceName ObjectName "NT AUTHORITY\LocalService" ""
    Set-ServiceEnvironment $(if ($ReuseExistingDatabase) { "" } else { $BootstrapToken })
    Start-Service -Name $ServiceName
    $service = Get-Service -Name $ServiceName -ErrorAction Stop
    if ($service.Status -ne "Running") { Stop-WithError "Service $ServiceName did not reach Running state." }
    Write-Ok "Service $ServiceName is running as LocalService and will start after reboot"
}
else {
    Write-Step "Starting api-server as a regular background process"
    $apiProcess = Start-DirectProcess $(if ($ReuseExistingDatabase) { "" } else { $BootstrapToken })
    Write-Ok "api-server started with PID $($apiProcess.Id); it will not auto-restart or survive a reboot unless you re-run with -InstallService"
}

try {
    Wait-ForHealthyServer $serverUrl
    if (-not $ReuseExistingDatabase) {
        Write-Step "Bootstrapping the first Admin account"
        Invoke-AdminBootstrap $serverUrl $AdminUsername $PlainAdminPassword $BootstrapToken

        # Remove the one-time token from the long-lived process/service after it
        # has served its only purpose, then re-run health and real-login checks.
        if ($InstallService) {
            Set-ServiceEnvironment ""
            Restart-Service -Name $ServiceName
        }
        else {
            Stop-Process -Id $apiProcess.Id -Force
            $apiProcess = Start-DirectProcess ""
        }
        Wait-ForHealthyServer $serverUrl
    }

    Write-Step "Verifying the configured account can really sign in"
    Confirm-Login $serverUrl $AdminUsername $PlainAdminPassword
}
finally {
    $PlainAdminPassword = $null
    Remove-Item Env:ONYX_BOOTSTRAP_TOKEN -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Setup complete. Staff and Admin clients on this PC can use $serverUrl; other LAN clients should use http://<this-PCs-LAN-IP>:$Port." -ForegroundColor Green
if (-not $InstallService) {
    Write-Host "The API is a regular background process. Re-run with -InstallService and a trusted NSSM path if you need automatic restart after logoff or reboot." -ForegroundColor Yellow
}
