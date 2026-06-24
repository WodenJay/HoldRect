# GitHub Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add GitHub Release distribution and a one-liner PowerShell install script for HoldRect.

**Architecture:** Two new files — a GitHub Actions workflow that builds and publishes `holdrect.exe` on tag push, and a `install.ps1` script that downloads the latest release exe and adds it to the user's PATH.

**Tech Stack:** GitHub Actions, PowerShell, `softprops/action-gh-release@v2`, `Swatinem/rust-cache@v2`

## Global Constraints

- Windows-only build, no cross-compile
- Binary name: `holdrect.exe`
- Install location: `$env:LOCALAPPDATA\HoldRect\`
- User-scope PATH only (no admin required)
- `<OWNER>` placeholder must be replaced with actual GitHub username before publishing

---

### Task 1: GitHub Actions Release Workflow

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Produces: GitHub Release with `holdrect.exe` asset when a `v*` tag is pushed

- [ ] **Step 1: Create workflow directory and file**

```bash
mkdir -p .github/workflows
```

- [ ] **Step 2: Write the release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  release:
    runs-on: windows-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Verify version matches tag
        shell: bash
        run: |
          CARGO_VERSION=$(grep -m1 '^version\s*=' Cargo.toml | sed 's/.*"\([0-9.]*\)".*/\1/')
          TAG_VERSION=${GITHUB_REF_NAME#v}
          if [ "$CARGO_VERSION" != "$TAG_VERSION" ]; then
            echo "::error::Cargo.toml version ($CARGO_VERSION) does not match tag ($TAG_VERSION)"
            exit 1
          fi

      - name: Verify no placeholder remains
        shell: bash
        run: |
          if grep -r '<OWNER>' . --include='*.ps1' --include='*.md' --include='*.yml' -l; then
            echo "::error::<OWNER> placeholder found in tracked files. Replace with actual GitHub username."
            exit 1
          fi

      - name: Cache cargo registry and build
        uses: Swatinem/rust-cache@v2

      - name: Build release
        run: cargo build --release

      - name: Create release
        uses: softprops/action-gh-release@v2
        with:
          files: target/release/holdrect.exe
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release workflow for GitHub distribution"
```

---

### Task 2: PowerShell Install Script

**Files:**
- Create: `install.ps1`

**Interfaces:**
- Consumes: GitHub Release at `https://github.com/<OWNER>/HoldRect/releases/latest/download/holdrect.exe`
- Produces: `holdrect.exe` at `$env:LOCALAPPDATA\HoldRect\holdrect.exe`, PATH entry added

- [ ] **Step 1: Write the install script**

Create `install.ps1`:

```powershell
#Requires -Version 5.1
try {
$ErrorActionPreference = 'Stop'

$Owner = '<OWNER>'  # Replace with actual GitHub username
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
    if (Test-Path $InstallPath) { Remove-Item $InstallPath -Force }
    Move-Item -Path $tempFile -Destination $InstallPath -Force
} catch {
    if (Test-Path $tempFile) { Remove-Item $tempFile -Force }
    Write-Error "Download failed: $_"
    exit 1
}

# Add to user PATH if not already present
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notlike "*$InstallDir*") {
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
```

- [ ] **Step 2: Verify script syntax locally**

```powershell
# Parse check — no execution
$errors = $null
[System.Management.Automation.Language.Parser]::ParseFile(
    (Resolve-Path '.\install.ps1').Path,
    [ref]$null,
    [ref]$errors
)
if ($errors.Count -gt 0) {
    $errors | ForEach-Object { Write-Error $_ }
    exit 1
} else {
    Write-Host "install.ps1 syntax OK" -ForegroundColor Green
}
```

Expected: `install.ps1 syntax OK`

- [ ] **Step 3: Commit**

```bash
git add install.ps1
git commit -m "feat: add PowerShell install script for GitHub distribution"
```

---

### Task 3: Update README with install instructions

**Files:**
- Modify: `README.md` (create if not exists)

- [ ] **Step 1: Add installation section to README**

If `README.md` exists, append the install section. If not, create it with minimal content.

Add this section (after any existing content):

```markdown
## Installation

### One-liner (recommended)

```powershell
irm https://raw.githubusercontent.com/<OWNER>/HoldRect/main/install.ps1 | iex
```

### Manual download

1. Go to [Releases](https://github.com/<OWNER>/HoldRect/releases/latest)
2. Download `holdrect.exe`
3. Run it
```

> Replace `<OWNER>` with actual GitHub username.

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add installation instructions to README"
```
