# Popup Apple-Style Light Translucent Design

**Date:** 2026-06-24
**Status:** Approved
**Scope:** Visual + animation polish for popup (status pill + cheatsheet card)

## Goal

Transform popup from dark-theme to Apple-style light translucent. Snappier animation feel.

## Color Changes (gdi_renderer.rs)

| Element | Current | New |
|---------|---------|-----|
| Status bg | `rgba(28,28,30, 224)` | `rgba(255,255,255, 200)` |
| Cheatsheet bg | `rgba(28,28,30, 235)` | `rgba(255,255,255, 216)` |
| Primary text | `0x00FFFFFF` | `0x001D1D1F` |
| Secondary text (key) | `0x00E5E5E5` | `0x001D1D1F` |
| Tertiary text (desc) | `0x00AEAEB2` | `0x0086868B` |
| Shadow base alpha (status) | 60 | 40 |
| Shadow base alpha (cheatsheet) | 80 | 60 |

## Animation Tuning (popup/mod.rs)

| Param | Current | New |
|-------|---------|-----|
| `SLIDE_IN_OMEGA` | 18.0 | 20.0 |
| `SLIDE_IN_ZETA` | 0.82 | 0.78 |
| `SLIDE_OUT_DURATION_MS` | 300 | 200 |
| `SLIDE_OUT_OMEGA` | 22.0 | 26.0 |

## What Does NOT Change

- Spring math (`animation.rs`)
- Layout constants (heights, padding, margins, radius)
- Font: Segoe UI
- Text content (already all English)
- State machine / phase transitions
- Config structure
- Overlay integration

## Files Modified

1. `src/popup/gdi_renderer.rs` — color constants + text colors + shadow alphas
2. `src/popup/mod.rs` — 4 animation constants

## Testing

Existing unit tests cover phase transitions and spring math. Visual verification by running app.
