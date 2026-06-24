# HoldRect

<p align="center">
  <img src="assets/HoldRect.png" alt="HoldRect logo" width="200">
</p>

> Hold modifier key + drag to draw a rainbow-striped rectangle anywhere on screen. Release to dismiss.

A lightweight, always-on screen highlighter for recordings and presentations. Zero mode switching, zero friction — just hold and drag.

<!-- TODO: Replace with actual demo GIF once recorded -->
<!-- Suggested: 5-second clip showing Alt+drag → rainbow rectangle → release → gone -->

## Why HoldRect?

- **🌈 Rainbow animated border** — gradient flows along the rectangle perimeter. Unique to HoldRect; no competitor offers animated borders.
- **⚡ Under 5 MB** — Rust native binary, no runtime, no Electron. Lightest tool in its class.
- **🖱️ Modifier + drag** — hold `Alt`, left-click drag, release. No mode switching, no toolbar clicks, no hotkey sequences.
- **📌 Pin & Spotlight** — press `1` during drag to pin the rectangle on screen; press `2` for spotlight (dim everything outside). Toggle anytime.
- **🪟 Transparent overlay** — `WS_EX_TRANSPARENT` keeps the rectangle visual-only; your clicks pass through to the app underneath.
- **🖥️ Windows now** — macOS and Linux support planned.

## Installation

### One-liner (recommended)

```powershell
irm https://raw.githubusercontent.com/<OWNER>/HoldRect/main/install.ps1 | iex
```

### Manual

1. Download `holdrect.exe` from [Releases](https://github.com/<OWNER>/HoldRect/releases/latest)
2. Run it — a tray icon appears, HoldRect is now listening
3. To exit: right-click the tray icon → **Exit**

## Quick Start

- `Alt` + drag: draw a rectangle
- `Alt` + `1` + drag: pinned rectangle (stays after release, `Esc` to clear)
- `Alt` + `2` + drag: spotlight (dims area outside the rectangle)
- `Alt` + `1` + `2` + drag: both
- Hold `` Alt + ` `` to see all shortcuts

## Configuration

HoldRect reads `~/.holdrect/config.toml`:

```toml
[general]
modifier = "Alt"              # Alt / Ctrl / Shift / Win
border_width = 4              # pixels
color = "rainbow"             # "rainbow" or hex like "#ff0000"
```

## How It Works

```
Modifier down → Left-click down → Drag (rectangle follows cursor)
                                    ├─ press 1: toggle Pin
                                    ├─ press 2: toggle Spotlight
                                    └─ press both: both active
              → Mouse up
                  ├─ Transient (default): rectangle vanishes
                  └─ Pinned: rectangle stays, Esc clears all
```

Each rectangle's Pin/Spotlight state is independent — drawing a new one resets to transient.

## Competitive Landscape

| Tool | Open Source | Memory | Animated Border | Zero Mode Switch | Cross-Platform |
|------|:-----------:|-------:|:---------------:|:----------------:|:--------------:|
| **HoldRect** | ✓ | **< 5 MB** | **✓ rainbow** | **✓** | Planned |
| Epic Pen | ✗ | ~20–50 MB | ✗ | ✗ | ✗ (Win) |
| ZoomIt | ✗ | ~10–15 MB | ✗ | ✗ (Ctrl+2) | ✗ (Win) |
| gInk | ✓ | ~15–30 MB | ✗ | ✗ | ✗ (Win) |
| Gromit-MPX | ✓ | ~5–10 MB | ✗ | ✗ | ✗ (Linux) |
| Fluor | ✓ | ~85 MB | ✗ | ✗ | ✗ (macOS) |

HoldRect's differentiators: rainbow flow animation, modifier+drag interaction, per-rect toggle with live status popup, and a Rust-native footprint under 5 MB.

## Building from Source

```bash
# Clone
git clone https://github.com/<OWNER>/HoldRect.git
cd HoldRect

# Build (Windows)
cargo build --release

# Run
cargo run --release
```

Requires Rust 1.75+ and Windows 10+.

## Contributing

Contributions welcome. Open an issue first for large changes.

For bug reports, include: Windows version, steps to reproduce, expected vs actual behavior.

## License

MIT
