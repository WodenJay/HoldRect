#Requires -Version 5.1
try {
$ErrorActionPreference = 'Stop'

$Owner = 'WodenJay'
$Repo = 'HoldRect'
$ExeName = 'holdrect.exe'
$InstallDir = Join-Path $env:LOCALAPPDATA 'HoldRect'
$InstallPath = Join-Path $InstallDir $ExeName
$DownloadUrl = "https://github.com/$Owner/$Repo/releases/latest/download/$ExeName"

# Check if holdrect is currently running
$running = Get-Process -Name 'holdrect' -ErrorAction SilentlyContinue
if ($running) {
    Write-Error "holdrect is currently running. Please close it first, then re-run this script."
    exit 1
}

# Create install directory
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# Download to temp file in same dir, then rename (NTFS same-volume rename is atomic)
$tempFile = Join-Path $InstallDir "holdrect.exe.tmp"
try {
    Write-Host "Downloading holdrect from GitHub..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $tempFile -UseBasicParsing

    # Rename into place (overwrites existing)
    Move-Item -Path $tempFile -Destination $InstallPath -Force
} catch {
    if (Test-Path $tempFile) { Remove-Item $tempFile -Force }
    Write-Error "Download failed: $_"
    exit 1
}

# Add to user PATH if not already present
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if (-not $userPath.Split(';').Contains($InstallDir)) {
    $newPath = if ($userPath) { "$userPath;$InstallDir" } else { $InstallDir }
    [Environment]::SetEnvironmentVariable('PATH', $newPath, 'User')
    # Also update current session
    $env:PATH = "$env:PATH;$InstallDir"
    Write-Host "Added $InstallDir to user PATH." -ForegroundColor Yellow
    Write-Host "Restart your terminal for PATH change to take effect in other sessions." -ForegroundColor Yellow
}

Write-Host "holdrect installed to $InstallPath" -ForegroundColor Green
Write-Host "Run 'holdrect' to start."
} catch {
    Write-Error "Installation failed: $_"
    exit 1
}
