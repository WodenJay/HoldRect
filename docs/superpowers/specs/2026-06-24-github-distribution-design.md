# GitHub Distribution Design

**Date:** 2026-06-24
**Scope:** Phase 1 — GitHub Release + PowerShell install script. Auto-update deferred to phase 2.

## Overview

Add two distribution channels for HoldRect (Windows-only Rust app):
1. **GitHub Release** — push a `v*` tag, CI builds `holdrect.exe` and publishes a release
2. **One-liner install** — `irm ... | iex` downloads exe and adds to PATH

## 1. GitHub Actions Workflow

**File:** `.github/workflows/release.yml`

**Trigger:** push tags matching `v*`

**Job:** single job on `windows-latest`:
1. Checkout repo
2. `cargo build --release` (uses existing optimized profile: lto, strip, panic=abort)
3. Upload `target/release/holdrect.exe` as release asset via `softprops/action-gh-release@v2`

**Release naming:** tag name becomes release title (e.g. tag `v0.5.0` → release `v0.5.0`).

**No cross-compile.** Windows-only build on Windows runner.

### Release workflow

```bash
# bump version in Cargo.toml, commit
git tag v0.5.0
git push origin v0.5.0
# CI auto-builds and publishes
```

## 2. Install Script

**File:** `install.ps1` (repo root)

**Usage:**
```powershell
irm https://raw.githubusercontent.com/<OWNER>/HoldRect/main/install.ps1 | iex
```

> **Note:** No git remote configured yet. Replace `<OWNER>` with actual GitHub username when repo is created.

**Behavior:**
1. Fetch latest release metadata from `https://api.github.com/repos/<OWNER>/HoldRect/releases/latest`
2. Find the asset named `holdrect.exe` in the response
3. Download to `$env:LOCALAPPDATA\HoldRect\holdrect.exe`
4. If `$env:LOCALAPPDATA\HoldRect` not in user PATH, append via `[Environment]::SetEnvironmentVariable('PATH', ..., 'User')`
5. Print success message with installed path

**Idempotent:** re-run overwrites exe, does not duplicate PATH entry.

**No admin required.** User-scope PATH only.

## Files to create

| File | Purpose |
|------|---------|
| `.github/workflows/release.yml` | CI build + release |
| `install.ps1` | One-liner install script |

## Deferred (phase 2)

- Auto-update: check GitHub releases on startup, download and self-replace exe
