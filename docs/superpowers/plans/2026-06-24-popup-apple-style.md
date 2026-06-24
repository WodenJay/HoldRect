# Popup Apple-Style Light Translucent — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform popup from dark-theme to Apple-style light translucent with snappier animation.

**Architecture:** Pure constant changes in 2 files — no new types, no new functions, no state machine changes.

**Tech Stack:** Rust, Win32 GDI (layered windows)

## Global Constraints

- `cargo test` max concurrency = 1
- No new dependencies
- All text remains English
- COLORREF values in BGR format (not RGB)

---

### Task 1: Update color constants in gdi_renderer.rs

**Files:**
- Modify: `src/popup/gdi_renderer.rs:17-23` (color constants)
- Modify: `src/popup/gdi_renderer.rs:171` (primary text COLORREF)
- Modify: `src/popup/gdi_renderer.rs:222` (key text COLORREF)
- Modify: `src/popup/gdi_renderer.rs:234` (desc text COLORREF)

**Interfaces:** No API changes. Internal constants only.

- [ ] **Step 1: Update BG color constants**

In `src/popup/gdi_renderer.rs`, replace lines 17-23:

```rust
const BG_R: u8 = 255;
const BG_G: u8 = 255;
const BG_B: u8 = 255;
const BG_A_STATUS: u8 = 230;
const BG_A_CHEATSHEET: u8 = 240;

const SHADOW_COLOR: (u8, u8, u8) = (0, 0, 0);
```

- [ ] **Step 2: Update primary text COLORREF (status popup)**

In `render_status`, line 171, change:
```rust
SetTextColor(self.mem_dc, COLORREF(0x00FFFFFF));
```
to:
```rust
SetTextColor(self.mem_dc, COLORREF(0x001F1D1D)); // #1D1D1F BGR
```

- [ ] **Step 3: Update key text COLORREF (cheatsheet)**

In `render_cheatsheet`, line 222, change:
```rust
SetTextColor(self.mem_dc, COLORREF(0x00E5E5E5)); // #E5E5E5
```
to:
```rust
SetTextColor(self.mem_dc, COLORREF(0x001F1D1D)); // #1D1D1F BGR
```

- [ ] **Step 4: Update desc text COLORREF (cheatsheet)**

In `render_cheatsheet`, line 234, change:
```rust
SetTextColor(self.mem_dc, COLORREF(0x00AEAEB2)); // #AEAEB2
```
to:
```rust
SetTextColor(self.mem_dc, COLORREF(0x008B8686)); // #86868B BGR
```

- [ ] **Step 5: Update shadow alpha values**

In `render_status`, line 163, change last arg from `60` to `55`:
```rust
paint_shadow(self.pixels, buf_w, buf_h, card_x + 2, card_y + 2, popup_w, popup_h, STATUS_RADIUS, SHADOW_COLOR, 55);
```

In `render_cheatsheet`, line 210 — keep at `80` (unchanged per spec).

- [ ] **Step 6: Run tests**

```bash
cargo test -p holdrect
```

Expected: all tests pass (no tests depend on color constants).

- [ ] **Step 7: Commit**

```bash
git add src/popup/gdi_renderer.rs
git commit -m "feat(popup): apple-style light translucent colors"
```

---

### Task 2: Update animation constants in mod.rs

**Files:**
- Modify: `src/popup/mod.rs:8-20` (animation constants)

**Interfaces:** No API changes. Internal constants only. Existing spring tests validate the math still works.

- [ ] **Step 1: Update animation constants**

In `src/popup/mod.rs`, replace lines 8-20:

```rust
const SLIDE_IN_DURATION_MS: u64 = 350;
const HOLD_DURATION_MS: u64 = 1000;
const SLIDE_OUT_DURATION_MS: u64 = 200;
const START_Y_OFFSET: f64 = -60.0;
const TARGET_Y_OFFSET: f64 = 0.0;

// Spring params: slide-in (slightly underdamped)
const SLIDE_IN_OMEGA: f64 = 20.0;
const SLIDE_IN_ZETA: f64 = 0.78;

// Spring params: slide-out (critically damped)
const SLIDE_OUT_OMEGA: f64 = 26.0;
const SLIDE_OUT_ZETA: f64 = 1.0;
```

- [ ] **Step 2: Update tests that hardcode old durations**

In the test `tick_sliding_in_to_holding_after_duration` (line 298), the elapsed `500ms` still exceeds new `350ms` — no change needed.

In `tick_holding_to_sliding_out_after_duration` (line 305), elapsed `1100ms` still exceeds `1000ms` — no change needed.

In `tick_sliding_out_to_hidden_after_duration` (line 314), elapsed `400ms` still exceeds new `200ms` — no change needed.

Verify: tests that set `started_at` to `Instant::now() - Duration::from_millis(X)` where X >= new constant will still pass.

- [ ] **Step 3: Run tests**

```bash
cargo test -p holdrect popup
```

Expected: all popup tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/popup/mod.rs
git commit -m "feat(popup): snappier spring animation params"
```
