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

Replaces `Vec<(i32,i32,i32,i32)>` in `AppState`.

### 2.2 Updated `AppState`

```rust
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<PinnedRect>,
    pub pinned_active: bool,
    pub spotlight_active: bool,  // NEW
}
```

Default: `spotlight_active: false`.

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

Both `pinned_active` and `spotlight_active` reset to false after rect is committed (mouse up or modifier release).

### 4.5 EscapePressed

Clear all pinned_rects AND reset both active flags to false.

### 4.6 Independence

- DigitPressed(1) only affects pinned_active
- DigitPressed(2) only affects spotlight_active
- Both can be true simultaneously = pinned + spotlight rect

## 5. Rendering Changes (src/overlay.rs)

### 5.1 Spotlight dimming

When any PinnedRect has `spotlight: true`:

1. Fill entire DIB with `(B=0, G=0, R=0, A=160)` — semi-transparent dark overlay
2. For each spotlight rect, clear interior pixels back to `(0,0,0,0)` — fully transparent = undimmed
3. Draw rainbow borders for ALL rects (pinned and active) on top

Non-spotlight rects: border only, no interior clearing.

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
dim_outside_spotlights(cache, ...)          // dim outside spotlight rects (pinned + active)
for rect in pinned_rects:
    draw_rect_in_dib(cache, ..., rect)      // border (overwrites dim at border)
if active_drawing:
    draw_rect_in_dib(cache, ..., active)    // active rect border
```

Active rect is included in spotlight dimming only when `state.spotlight_active` is true.

## 6. Tests

### 6.1 Hook tests (src/hook.rs)

- digit_2_modifier_held_emits_digit_pressed_2
- digit_2_modifier_not_held_returns_none

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

### 6.3 Render tests (src/overlay.rs)

- dim_outside_spotlights_fills_dark_outside_rect
- dim_outside_spotlights_clears_interior
- dim_outside_spotlights_noop_when_no_spotlight_rects

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
