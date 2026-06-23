# Spotlight Mode Spec

> Digit 2 toggle: dim area outside drawn rect. Per-rect, combinable with pinned.

---

## 1. Goal

Add Spotlight mode to HoldRect. When enabled, the area outside the drawn rectangle dims (semi-transparent dark overlay), creating a spotlight/cone-of-vision effect. Follows same per-rect toggle pattern as pinned mode.

## 2. Data Model Changes

### 2.1 New struct: `PinnedRect` (src/state.rs)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub spotlight: bool,
}
```

Replaces `Vec<(i32,i32,i32,i32)>` in `AppState`. All existing tuple-destructuring sites in `overlay.rs` (e.g. line 249 `&(x0, y0, x1, y1)`) must be updated to use struct fields.

### 2.2 Updated `AppState`

```rust
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<PinnedRect>,
    pub pinned_active: bool,
    pub spotlight_active: bool,  // NEW
}
```

Default impl updated: add `spotlight_active: false`. `PinnedRect` does not need `Default` — only `Vec<PinnedRect>` is constructed via `Vec::new()`.

### 2.3 Migration note

`process_event` currently returns a 3-tuple `(drawing, pinned_active, pinned_rects)`. With `spotlight_active` added, every match arm (13+ arms) must expand to a 4-tuple. Missing any arm will silently pass through stale state. This is a pervasive mechanical change.

## 3. Hook Changes (src/hook.rs)

`decide_keyboard`: add digit 2 handling.

```rust
// After existing digit 1 check:
if modifier_held && vk_code == 0x32 {
    return Some(InputEvent::DigitPressed(2));
}
```

`DigitPressed(u8)` enum variant already generic — no InputEvent changes needed.

## 4. State Machine Changes (src/state.rs)

### 4.1 DigitPressed(2) toggle

Same pattern as DigitPressed(1):

```
(Armed, DigitPressed(2))      → toggle spotlight_active, drawing unchanged
(Drawing{..}, DigitPressed(2)) → toggle spotlight_active, drawing unchanged
(Idle, DigitPressed(2))        → no-op
```

### 4.2 MouseUp with pinned_active

Push `PinnedRect { x0, y0, x1, y1, spotlight: spotlight_active }`.

### 4.3 Modifier release with pinned_active

Same as mouse up — push PinnedRect with current spotlight flag.

### 4.4 Per-rect reset

Both `pinned_active` and `spotlight_active` reset to false after rect is committed (mouse up or modifier release). This means the user must re-toggle digit 1 (pinned) and digit 2 (spotlight) for each new rect. This matches the existing pinned mode pattern — toggles are per-rect, not persistent across draws.

### 4.5 EscapePressed

Clear all pinned_rects AND reset both active flags to false.

### 4.6 Independence

- DigitPressed(1) only affects `pinned_active`
- DigitPressed(2) only affects `spotlight_active`
- Both can be true simultaneously = pinned + spotlight rect
- Both reset per-rect: user must re-toggle for each new rect

## 5. Rendering Changes (src/overlay.rs)

### 5.1 Spotlight dimming

When any PinnedRect has `spotlight: true` (or active drawing has `spotlight_active = true`):

1. Fill entire DIB with `(B=0, G=0, R=0, A=160)` — semi-transparent dark overlay (~63% opacity, tunable)
2. For each spotlight rect, clear full rect area `(x0..=x1, y0..=y1)` back to `(0,0,0,0)` — fully transparent = undimmed
3. Draw rainbow borders for ALL rects (pinned and active) on top

Non-spotlight rects: border only, no interior clearing.

**Pixel writing**: `dim_outside_spotlights` writes directly to the DIB pixel buffer in BGRA byte order (buf[offset]=b, buf[offset+1]=g, buf[offset+2]=r, buf[offset+3]=a). Cannot reuse existing `set_pixel` helper which hardcodes alpha to 255.

### 5.2 New function

```rust
fn dim_outside_spotlights(
    cache: &mut DibCache,
    width: i32,
    height: i32,
    rects: &[PinnedRect],
    window_left: i32,
    window_top: i32,
)
```

Called before `draw_rect_in_dib` loop in `render()`. Only applies to rects with `spotlight=true`.

### 5.3 Live preview during draw

When user is actively drawing with `spotlight_active = true`, apply spotlight dimming to the current rect-in-progress as live preview. Check `state.spotlight_active` in render() and include active rect in dim calculation.

### 5.4 Render order

```
clear_dib_pixels(cache, width, height)     // all transparent

// Build spotlight rects list
spotlight_rects = pinned_rects.filter(spotlight)
if active_drawing && spotlight_active:
    append active rect bounds to spotlight_rects

dim_outside_spotlights(cache, ..., spotlight_rects)

for rect in pinned_rects:
    draw_rect_in_dib(cache, ..., rect)      // border (overwrites dim at border)
if active_drawing:
    draw_rect_in_dib(cache, ..., active)    // active rect border
```

## 6. Tests

### 6.1 Hook tests (src/hook.rs)

- digit_2_modifier_held_emits_digit_pressed_2
- digit_2_modifier_not_held_returns_none
- digit_2_key_up_returns_none (mirror digit 1 pattern)

**Existing test to update**: `digit_2_modifier_held_returns_none` at hook.rs:585 currently asserts digit 2 returns None. Must be removed or changed to assert `DigitPressed(2)`.

### 6.2 State tests (src/state.rs)

- armed_digit_2_toggles_spotlight_active
- drawing_digit_2_toggles_spotlight_active
- idle_digit_2_is_noop
- drawing_mouse_up_pinned_spotlight_pushes_pinned_rect_with_spotlight_true
- drawing_mouse_up_pinned_no_spotlight_pushes_spotlight_false
- spotlight_active_resets_after_mouse_up
- escape_resets_spotlight_active
- modifier_release_resets_spotlight_active
- pinned_and_spotlight_independent (1 then 2, both true)
- digit_1_does_not_affect_spotlight
- digit_2_does_not_affect_pinned
- multiple_spotlight_rects_accumulate (toggle 2 → draw → mouse up → toggle 2 → draw → verify both rects have spotlight=true)
- mixed_spotlight_and_non_spotlight (toggle 1 only → draw rect A, toggle 1+2 → draw rect B, verify A.spotlight=false, B.spotlight=true)

### 6.3 Render tests (src/overlay.rs)

- dim_outside_spotlights_fills_dark_outside_rect
- dim_outside_spotlights_clears_interior
- dim_outside_spotlights_noop_when_no_spotlight_rects
- dim_outside_spotlights_mixed_spotlight_and_non_spotlight (only spotlight rect interiors cleared, non-spotlight interiors stay dimmed)
- dim_outside_spotlights_overlapping_rects (clearing is idempotent, no re-dim at overlap)

## 7. Scope

In scope:
- Spotlight state machine + toggle
- Spotlight rendering (dim outside rect)
- Tests for all layers

Out of scope (future iterations):
- Toast popup for digit toggle feedback
- Hotkey help card
- Tray menu mode switch
- Config default_mode field
