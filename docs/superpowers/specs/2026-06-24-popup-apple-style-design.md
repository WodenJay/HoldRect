# Popup Apple-Style Light Translucent Design

**Date:** 2026-06-24
**Status:** Approved
**Scope:** Visual + animation polish for popup (status pill + cheatsheet card)

## Goal

Transform popup from dark-theme to Apple-style light translucent. Snappier animation feel.

## Color Changes (gdi_renderer.rs)

| Element | Current | New |
|---------|---------|-----|
| Status bg | `rgba(28,28,30, 224)` | `rgba(255,255,255, 230)` |
| Cheatsheet bg | `rgba(28,28,30, 235)` | `rgba(255,255,255, 240)` |
| Primary text | `0x00FFFFFF` (COLORREF BGR) | `0x001F1D1D` (#1D1D1F → BGR) |
| Secondary text (key) | `0x00E5E5E5` | `0x001F1D1D` (same; weight distinguishes) |
| Tertiary text (desc) | `0x00AEAEB2` | `0x008B8686` (#86868B → BGR) |
| Shadow base alpha (status) | 60 | 55 |
| Shadow base alpha (cheatsheet) | 80 | 80 |

## Animation Tuning (popup/mod.rs)

| Param | Current | New | Note |
|-------|---------|-----|------|
| `SLIDE_IN_OMEGA` | 18.0 | 20.0 | |
| `SLIDE_IN_ZETA` | 0.82 | 0.78 | Slightly more overshoot |
| `SLIDE_IN_DURATION_MS` | 400 | 350 | Match faster spring |
| `SLIDE_OUT_OMEGA` | 22.0 | 26.0 | |
| `SLIDE_OUT_ZETA` | 1.0 | 1.0 | Unchanged (critically damped) |
| `SLIDE_OUT_DURATION_MS` | 300 | 200 | |

## What Does NOT Change

- Spring math (`animation.rs`)
- Layout constants (heights, padding, margins, radius)
- Font: Segoe UI
- Text content (already all English)
- State machine / phase transitions
- Config structure
- Overlay integration

## Design Notes

- **No backdrop blur:** GDI layered windows have no vibrancy/blur. Card alpha raised to 230/240 for legibility on light wallpapers. Shadow kept at near-original alpha (55/80) so card edges remain visible.
- **Key vs desc color:** Both use `#1D1D1F`. Distinction is by font weight only (semibold vs normal) — matches Apple cheatsheet style.
- **COLORREF byte order:** All hex values above are in Windows COLORREF (BGR) format. `#1D1D1F` → `0x001F1D1D`.
- **Timeout guard:** `SLIDE_IN_DURATION_MS` is a safety margin, not the visual duration. With omega=20 the spring settles ~300ms; 350ms guard is sufficient.

## Files Modified

1. `src/popup/gdi_renderer.rs` — color constants + text colors + shadow alphas
2. `src/popup/mod.rs` — 5 animation constants (omega, zeta, duration × 2 phases)

## Testing

Existing unit tests cover phase transitions and spring math. Visual verification by running app.
