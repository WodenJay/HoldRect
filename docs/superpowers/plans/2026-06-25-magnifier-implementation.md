# Magnifier Feature Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Alt+3 magnifier feature — circular magnifier follows cursor, scroll wheel adjusts zoom, separate WS_POPUP window with BitBlt screen capture.

**Architecture:** New `src/magnifier.rs` module owns a WS_POPUP layered window. State machine extended with `magnifier_active` + `zoom_level`. Hook extended with DigitPressed(3) and WM_MOUSEWHEEL. Overlay integrates magnifier rendering after its own render pass.

**Tech Stack:** Rust, Win32 API (BitBlt, StretchBlt, UpdateLayeredWindow, GDI path clipping), existing DibCache pattern.

## Global Constraints

- `cargo test` with `CARGO_BUILD_JOBS=1` (low memory)
- TDD: write failing test → verify red → implement → verify green → commit
- No mock/dead code, no `#[allow(dead_code)]`
- Each task ends with independently testable deliverable + commit
- Existing tests must not break (130+ tests in state.rs, 40+ in hook.rs)

---

### Task 1: state.rs — InputEvent + AppState + process_event (TDD)

**Files:**
- Modify: `src/state.rs`

**Interfaces:**
- Produces: `InputEvent::ScrollUp`, `InputEvent::ScrollDown`, `AppState.magnifier_active: bool`, `AppState.zoom_level: f64`, updated `process_event()`

- [ ] **Step 1: Write failing tests for new InputEvent variants**

Add to `src/state.rs` tests section:

```rust
// --- Magnifier mode: DigitPressed(3) toggle ---

#[test]
fn armed_digit_3_toggles_magnifier_active() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(3));
    assert!(next.magnifier_active);
    assert_eq!(next.drawing, DrawingState::Armed);
}

#[test]
fn armed_digit_3_toggle_off() {
    let state = AppState { drawing: DrawingState::Armed, magnifier_active: true, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(3));
    assert!(!next.magnifier_active);
}

#[test]
fn drawing_digit_3_toggles_magnifier_active() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::DigitPressed(3));
    assert!(next.magnifier_active);
    assert_eq!(next.drawing, DrawingState::Drawing { start: (10, 20), current: (50, 80) });
}

#[test]
fn idle_digit_3_is_noop() {
    let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(3));
    assert!(!next.magnifier_active);
    assert_eq!(next.drawing, DrawingState::Idle);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib state::tests::armed_digit_3_toggles_magnifier_active 2>&1`
Expected: FAIL — `magnifier_active` field not found on `AppState`

- [ ] **Step 3: Write failing tests for scroll zoom**

```rust
// --- Magnifier zoom: ScrollUp/ScrollDown ---

#[test]
fn magnifier_scroll_up_increases_zoom() {
    let state = AppState { drawing: DrawingState::Armed, magnifier_active: true, zoom_level: 2.0, ..Default::default() };
    let next = process_event(&state, &InputEvent::ScrollUp);
    assert!((next.zoom_level - 2.5).abs() < f64::EPSILON);
}

#[test]
fn magnifier_scroll_down_decreases_zoom() {
    let state = AppState { drawing: DrawingState::Armed, magnifier_active: true, zoom_level: 2.0, ..Default::default() };
    let next = process_event(&state, &InputEvent::ScrollDown);
    assert!((next.zoom_level - 1.5).abs() < f64::EPSILON);
}

#[test]
fn magnifier_zoom_clamped_at_8_0() {
    let state = AppState { drawing: DrawingState::Armed, magnifier_active: true, zoom_level: 8.0, ..Default::default() };
    let next = process_event(&state, &InputEvent::ScrollUp);
    assert!((next.zoom_level - 8.0).abs() < f64::EPSILON);
}

#[test]
fn magnifier_zoom_clamped_at_1_5() {
    let state = AppState { drawing: DrawingState::Armed, magnifier_active: true, zoom_level: 1.5, ..Default::default() };
    let next = process_event(&state, &InputEvent::ScrollDown);
    assert!((next.zoom_level - 1.5).abs() < f64::EPSILON);
}

#[test]
fn scroll_without_magnifier_active_is_noop() {
    let state = AppState { drawing: DrawingState::Armed, magnifier_active: false, zoom_level: 2.0, ..Default::default() };
    let next = process_event(&state, &InputEvent::ScrollUp);
    assert!((next.zoom_level - 2.0).abs() < f64::EPSILON);
    assert!(!next.magnifier_active);
}
```

- [ ] **Step 4: Write failing tests for modifier release + escape resets**

```rust
// --- Magnifier: modifier release resets magnifier_active, preserves zoom ---

#[test]
fn modifier_release_resets_magnifier_active_preserves_zoom() {
    let state = AppState {
        drawing: DrawingState::Armed,
        magnifier_active: true,
        zoom_level: 4.0,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
    assert!(!next.magnifier_active);
    assert!((next.zoom_level - 4.0).abs() < f64::EPSILON, "zoom_level must be preserved");
    assert_eq!(next.drawing, DrawingState::Idle);
}

#[test]
fn escape_resets_magnifier_active() {
    let state = AppState {
        drawing: DrawingState::Armed,
        magnifier_active: true,
        zoom_level: 3.0,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::EscapePressed);
    assert!(!next.magnifier_active);
    // zoom_level preserved
    assert!((next.zoom_level - 3.0).abs() < f64::EPSILON);
}

// --- Magnifier independence with pinned/spotlight ---

#[test]
fn magnifier_and_pinned_independent() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(1));
    let next = process_event(&next, &InputEvent::DigitPressed(3));
    assert!(next.pinned_active);
    assert!(next.magnifier_active);
}

#[test]
fn magnifier_and_spotlight_independent() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(2));
    let next = process_event(&next, &InputEvent::DigitPressed(3));
    assert!(next.spotlight_active);
    assert!(next.magnifier_active);
}
```

- [ ] **Step 5: Run all new tests to verify they fail**

Run: `cargo test --lib state::tests 2>&1 | head -50`
Expected: Compilation failure — `magnifier_active` and `zoom_level` not found on `AppState`, `ScrollUp`/`ScrollDown` not found on `InputEvent`

- [ ] **Step 6: Add InputEvent variants**

In `src/state.rs`, add to the `InputEvent` enum:

```rust
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    DigitPressed(u8),
    EscapePressed,
    ToggleHelp,
    HideHelp,
    ScrollUp,    // NEW
    ScrollDown,  // NEW
}
```

- [ ] **Step 7: Add AppState fields**

```rust
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<PinnedRect>,
    pub pinned_active: bool,
    pub spotlight_active: bool,
    pub magnifier_active: bool,  // NEW
    pub zoom_level: f64,         // NEW
}
```

Update `Default` impl:
```rust
impl Default for AppState {
    fn default() -> Self {
        Self {
            drawing: DrawingState::Idle,
            pinned_rects: Vec::new(),
            pinned_active: false,
            spotlight_active: false,
            magnifier_active: false,
            zoom_level: 2.0,
        }
    }
}
```

- [ ] **Step 8: Extend process_event**

Expand the destructured return tuple from 4 to 6 fields:
```rust
let (drawing, pinned_active, spotlight_active, magnifier_active, zoom_level, pinned_rects) = match (&state.drawing, event) {
```

Add new arms **before** the wildcard `_ =>` arm:

```rust
// --- DigitPressed(3) magnifier toggle (only in Armed or Drawing) ---
(DrawingState::Armed, InputEvent::DigitPressed(3)) => {
    (state.drawing.clone(), state.pinned_active, state.spotlight_active, !state.magnifier_active, state.zoom_level, state.pinned_rects.clone())
}
(DrawingState::Drawing { .. }, InputEvent::DigitPressed(3)) => {
    (state.drawing.clone(), state.pinned_active, state.spotlight_active, !state.magnifier_active, state.zoom_level, state.pinned_rects.clone())
}

// --- ScrollUp/ScrollDown zoom adjustment ---
(_, InputEvent::ScrollUp) if state.magnifier_active => {
    (state.drawing.clone(), state.pinned_active, state.spotlight_active, state.magnifier_active, (state.zoom_level + 0.5).min(8.0), state.pinned_rects.clone())
}
(_, InputEvent::ScrollDown) if state.magnifier_active => {
    (state.drawing.clone(), state.pinned_active, state.spotlight_active, state.magnifier_active, (state.zoom_level - 0.5).max(1.5), state.pinned_rects.clone())
}
```

**Extend existing arms** — add `state.magnifier_active` and `state.zoom_level` to every existing arm's tuple. Key changes:
- `EscapePressed` arm (line 83): set `magnifier_active: false` but keep `state.zoom_level`
- `ModifierChanged { pressed: false }` arms (lines 116, 120): set `magnifier_active: false` but keep `state.zoom_level`
- All other arms: pass through `state.magnifier_active, state.zoom_level`

Update the final AppState construction:
```rust
AppState { drawing, pinned_rects, pinned_active, spotlight_active, magnifier_active, zoom_level }
```

- [ ] **Step 9: Run all state tests**

Run: `cargo test --lib state::tests 2>&1`
Expected: ALL PASS (new + existing ~130 tests)

- [ ] **Step 10: Commit**

```bash
git add src/state.rs
git commit -m "feat(state): add magnifier_active, zoom_level, ScrollUp/ScrollDown to state machine"
```

---

### Task 2: hook.rs — DigitPressed(3) + WM_MOUSEWHEEL (TDD)

**Files:**
- Modify: `src/hook.rs`

**Interfaces:**
- Consumes: `InputEvent::ScrollUp`, `InputEvent::ScrollDown` (from Task 1)
- Produces: `decide_keyboard` emits `DigitPressed(3)` for vk_code 0x33, `decide_mouse` emits `ScrollUp`/`ScrollDown` for WM_MOUSEWHEEL

- [ ] **Step 1: Write failing tests for DigitPressed(3)**

Add to `src/hook.rs` tests section:

```rust
// --- Digit 3 (magnifier) ---

#[test]
fn digit_3_modifier_held_emits_digit_pressed_3() {
    let result = decide_keyboard(0x33, true, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, Some(InputEvent::DigitPressed(3)));
}

#[test]
fn digit_3_modifier_not_held_returns_none() {
    let result = decide_keyboard(0x33, true, &[0x12, 0xA4, 0xA5], false);
    assert_eq!(result, None);
}

#[test]
fn digit_3_key_up_returns_none() {
    let result = decide_keyboard(0x33, false, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib hook::tests::digit_3_modifier_held_emits_digit_pressed_3 2>&1`
Expected: FAIL — `decide_keyboard(0x33, ...)` returns `None`

- [ ] **Step 3: Add DigitPressed(3) to decide_keyboard**

In `src/hook.rs`, inside the `if is_key_down { ... }` block, after the `vk_code == 0x32` arm:

```rust
if modifier_held && vk_code == 0x33 {
    return Some(InputEvent::DigitPressed(3));
}
```

- [ ] **Step 4: Run digit 3 tests**

Run: `cargo test --lib hook::tests::digit_3 2>&1`
Expected: ALL PASS

- [ ] **Step 5: Write failing tests for WM_MOUSEWHEEL**

```rust
// --- WM_MOUSEWHEEL (magnifier zoom) ---

#[test]
fn mouse_scroll_up_modifier_held_emits_scroll_up() {
    // WM_MOUSEWHEEL = 0x020A, positive delta = scroll up
    // decide_mouse needs a new parameter for magnifier_active
    // For now test with existing signature — will need signature change
}

#[test]
fn mouse_scroll_modifier_not_held_is_none() {
    let (event, suppress) = decide_mouse(0x020A, (100, 200), false, false, false);
    assert_eq!(event, None);
    assert!(!suppress);
}
```

**Important:** `decide_mouse` needs two new parameters: `magnifier_active: bool` (to decide whether to suppress scroll) and `wparam: isize` (to extract WM_MOUSEWHEEL delta). Update signature:

```rust
pub(crate) fn decide_mouse(
    msg: u32,
    pt: (i32, i32),
    should_suppress: bool,
    drag_in_progress: bool,
    modifier_held: bool,
    magnifier_active: bool,  // NEW
    wparam: isize,           // NEW — for WM_MOUSEWHEEL delta
) -> (Option<InputEvent>, bool)
```

Update **ALL** existing callers of `decide_mouse`:
- `mouse_hook_proc` in hook.rs: pass `MAGNIFIER_ACTIVE.load(Ordering::Relaxed)` and `w_param.0 as isize`
- All 30+ test calls in hook.rs: add `false, 0` as last two args
- All 15+ test calls in overlay.rs: add `false, 0` as last two args

**Breaking test fix:** Existing test `mouse_unknown_message_during_drag_is_noop` uses `WM_MOUSEWHEEL` (0x020A) as an "unknown message". With the new WM_MOUSEWHEEL handling (before drag_in_progress check), this test will now match the scroll handler. Update the test to use a truly unknown message code (e.g., `0x020B` = WM_XBUTTONDOWN) instead.

- [ ] **Step 6: Write complete scroll tests**

```rust
#[test]
fn mouse_scroll_up_modifier_held_magnifier_active_emits_scroll_up() {
    let (event, suppress) = decide_mouse(0x020A, (100, 200), true, false, true, true);
    assert_eq!(event, Some(InputEvent::ScrollUp));
    assert!(suppress, "scroll should be suppressed when magnifier active");
}

#[test]
fn mouse_scroll_down_modifier_held_magnifier_active_emits_scroll_down() {
    // negative delta = scroll down, but decide_mouse only sees the message code
    // WM_MOUSEWHEEL = 0x020A for both directions; delta is in wParam
    // We need to extract delta from wParam in the real hook, but decide_mouse
    // receives the raw msg. Need to split into WM_MOUSEWHEEL_UP / WM_MOUSEWHEEL_DOWN
    // OR pass delta as a separate parameter.
}
```

**Design decision:** `WM_MOUSEWHEEL` carries the delta in the high word of wParam. The `decide_mouse` function receives `msg: u32` but not wParam. Two options:
1. Change `decide_mouse` signature to include `wparam: u32`
2. In `mouse_hook_proc`, extract delta before calling `decide_mouse`, and pass a synthetic msg

**Recommended:** Option 1 — add `wparam: isize` parameter to `decide_mouse`. In the hook, pass `w_param.0 as isize`. In tests, pass the delta directly.

Updated signature:
```rust
pub(crate) fn decide_mouse(
    msg: u32,
    pt: (i32, i32),
    should_suppress: bool,
    drag_in_progress: bool,
    modifier_held: bool,
    magnifier_active: bool,
    wparam: isize,  // NEW — for WM_MOUSEWHEEL delta extraction
) -> (Option<InputEvent>, bool)
```

- [ ] **Step 7: Implement WM_MOUSEWHEEL in decide_mouse**

Add before the `drag_in_progress` early-return block (spec requirement: WM_MOUSEWHEEL must not be blocked by drag):

```rust
// Handle scroll wheel BEFORE drag_in_progress check
if msg == WM_MOUSEWHEEL {
    if modifier_held && magnifier_active {
        let delta = ((wparam >> 16) & 0xFFFF) as i16 as i32; // GET_WHEEL_DELTA_WPARAM
        let event = if delta > 0 { InputEvent::ScrollUp } else { InputEvent::ScrollDown };
        return (Some(event), true); // suppress scroll when magnifier active
    }
    return (None, false); // pass through when magnifier not active
}
```

- [ ] **Step 8: Update all decide_mouse callers with new parameters**

- `mouse_hook_proc`: pass `magnifier_active` from a new `MAGNIFIER_ACTIVE` AtomicBool, pass `w_param.0 as isize`
- All test calls: add `false, 0` (magnifier_active=false, wparam=0) as last two args

- [ ] **Step 9: Write final scroll tests**

```rust
#[test]
fn mouse_scroll_up_magnifier_active() {
    let wparam: isize = (120i32 << 16) as isize; // positive delta = scroll up
    let (event, suppress) = decide_mouse(0x020A, (100, 200), true, false, true, true, wparam);
    assert_eq!(event, Some(InputEvent::ScrollUp));
    assert!(suppress);
}

#[test]
fn mouse_scroll_down_magnifier_active() {
    let wparam: isize = (-120i32 as u32 as isize) << 16; // negative delta
    let (event, suppress) = decide_mouse(0x020A, (100, 200), true, false, true, true, wparam);
    assert_eq!(event, Some(InputEvent::ScrollDown));
    assert!(suppress);
}

#[test]
fn mouse_scroll_magnifier_not_active_passes_through() {
    let wparam: isize = (120i32 << 16) as isize;
    let (event, suppress) = decide_mouse(0x020A, (100, 200), true, false, true, false, wparam);
    assert_eq!(event, None);
    assert!(!suppress);
}

#[test]
fn mouse_scroll_modifier_not_held_passes_through() {
    let wparam: isize = (120i32 << 16) as isize;
    let (event, suppress) = decide_mouse(0x020A, (100, 200), false, false, false, false, wparam);
    assert_eq!(event, None);
    assert!(!suppress);
}

#[test]
fn mouse_scroll_during_drag_magnifier_active_still_works() {
    let wparam: isize = (120i32 << 16) as isize;
    let (event, suppress) = decide_mouse(0x020A, (100, 200), true, true, true, true, wparam);
    assert_eq!(event, Some(InputEvent::ScrollUp));
    assert!(suppress);
}
```

- [ ] **Step 10: Run all hook tests**

Run: `cargo test --lib hook::tests 2>&1`
Expected: ALL PASS

- [ ] **Step 11: Commit**

```bash
git add src/hook.rs
git commit -m "feat(hook): add DigitPressed(3) and WM_MOUSEWHEEL handling for magnifier"
```

---

### Task 3: magnifier.rs — MagnifierWindow struct + circular_perimeter_position (TDD)

**Files:**
- Create: `src/magnifier.rs`
- Modify: `src/main.rs` (add `mod magnifier;`)

**Interfaces:**
- Produces: `MagnifierWindow::new(diameter) -> Self`, `MagnifierWindow::render(cursor_pos, zoom, color_mode, time_offset)`, `MagnifierWindow::hide()`, `circular_perimeter_position(x, y, cx, cy) -> f32`

- [ ] **Step 1: Create src/magnifier.rs with circular_perimeter_position + tests**

```rust
use std::f64::consts::PI;

/// Default magnifier diameter in pixels
pub const MAGNIFIER_DIAMETER: i32 = 350;

/// Rainbow border width in pixels
const BORDER_WIDTH: i32 = 4;

/// Compute perimeter position (0.0..1.0) around a circle for a point on the border.
/// Uses atan2 angle, starting from right (3 o'clock), going clockwise.
pub fn circular_perimeter_position(x: i32, y: i32, cx: i32, cy: i32) -> f32 {
    let dx = (x - cx) as f64;
    let dy = (y - cy) as f64;
    let angle = dy.atan2(dx); // -PI..PI
    let normalized = (angle + PI) / (2.0 * PI); // 0..1
    normalized as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_perimeter_right_is_zero() {
        let pos = circular_perimeter_position(100, 50, 50, 50);
        assert!((pos - 0.5).abs() < 0.01, "right (0°) should map to ~0.5, got {}", pos);
    }

    #[test]
    fn circular_perimeter_top_is_quarter() {
        let pos = circular_perimeter_position(50, 0, 50, 50);
        assert!((pos - 0.25).abs() < 0.01, "top (270°/−90°) should map to ~0.25, got {}", pos);
    }

    #[test]
    fn circular_perimeter_wraps_around() {
        let pos1 = circular_perimeter_position(50, 50 + 10, 50, 50); // bottom
        let pos2 = circular_perimeter_position(50, 50 - 10, 50, 50); // top
        assert!((pos1 - pos2).abs() > 0.4, "opposite sides should be ~0.5 apart");
    }

    #[test]
    fn circular_perimeter_same_point_is_defined() {
        // When point == center, atan2(0,0) is defined (0.0 in Rust)
        let pos = circular_perimeter_position(50, 50, 50, 50);
        assert!((0.0..=1.0).contains(&pos));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib magnifier::tests 2>&1`
Expected: ALL PASS

- [ ] **Step 3: Add MagnifierWindow struct skeleton**

```rust
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Magnifier window — separate WS_POPUP with circular clip, screen capture, zoom.
#[cfg(windows)]
pub struct MagnifierWindow {
    hwnd: HWND,
    diameter: i32,
}

#[cfg(windows)]
impl MagnifierWindow {
    pub fn new(diameter: i32) -> Self {
        // Window creation will be implemented in Task 4
        todo!("MagnifierWindow::new")
    }

    pub fn render(&mut self, cursor_pos: (i32, i32), zoom: f64, color_mode: &crate::config::ColorMode, time_offset: f32) {
        todo!("MagnifierWindow::render")
    }

    pub fn hide(&self) {
        todo!("MagnifierWindow::hide")
    }
}
```

- [ ] **Step 4: Add mod magnifier to main.rs**

In `src/main.rs`, add after the existing mod declarations:

```rust
mod magnifier;
```

- [ ] **Step 5: Verify compilation**

Run: `cargo build 2>&1`
Expected: PASS (todo!() compiles but panics at runtime)

- [ ] **Step 6: Commit**

```bash
git add src/magnifier.rs src/main.rs
git commit -m "feat(magnifier): add module skeleton with circular_perimeter_position"
```

---

### Task 4: magnifier.rs — Window creation + render cycle

**Files:**
- Modify: `src/magnifier.rs`

**Interfaces:**
- Consumes: `circular_perimeter_position` (from Task 3), `DibCache` pattern from overlay.rs, `hsv_to_rgb` from overlay.rs
- Produces: Working `MagnifierWindow::new()`, `render()`, `hide()`

- [ ] **Step 1: Implement MagnifierWindow::new()**

```rust
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Foundation::{HWND, RECT, WPARAM, LPARAM, LRESULT};

impl MagnifierWindow {
    pub fn new(diameter: i32, overlay_hwnd: HWND) -> Self {
        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
                windows::core::w!("STATIC"),
                windows::core::w!("HoldRect Magnifier"),
                WS_POPUP,
                0, 0, diameter, diameter,
                None, None, None, None,
            ).expect("Failed to create magnifier window");

            // Set overlay as owner for Z-order stacking
            SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, overlay_hwnd.0 as isize);

            Self { hwnd, diameter }
        }
    }
}
```

- [ ] **Step 2: Implement hide()**

```rust
pub fn hide(&self) {
    unsafe {
        let _ = ShowWindow(self.hwnd, SW_HIDE);
    }
}
```

- [ ] **Step 3: Implement render() — screen capture + zoom + circular clip + border + text**

Follow the render cycle from the spec:

```rust
pub fn render(&mut self, cursor_pos: (i32, i32), zoom: f64, color_mode: &crate::config::ColorMode, time_offset: f32) {
    unsafe {
        let d = self.diameter;
        let r = d / 2;

        // 1. Hide to avoid capturing ourselves
        let _ = ShowWindow(self.hwnd, SW_HIDE);

        // 2. Position window at cursor
        let x = cursor_pos.0 - r;
        let y = cursor_pos.1 - r;
        let _ = SetWindowPos(self.hwnd, None, x, y, d, d, SWP_NOACTIVATE | SWP_NOZORDER);

        // 3. Capture screen region
        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let capture_w = (d as f64 / zoom) as i32;
        let capture_h = (d as f64 / zoom) as i32;
        let src_x = cursor_pos.0 - capture_w / 2;
        let src_y = cursor_pos.1 - capture_h / 2;

        // Create capture bitmap
        let cap_bmp = CreateCompatibleBitmap(screen_dc, capture_w, capture_h);
        let old_bmp = SelectObject(mem_dc, cap_bmp.into());
        let _ = BitBlt(mem_dc, 0, 0, capture_w, capture_h, Some(screen_dc), src_x, src_y, SRCCOPY);

        // 4. Create DIB for the magnifier window content
        let dib_dc = CreateCompatibleDC(Some(screen_dc));
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: d,
                biHeight: -d, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: (d * d * 4) as u32,
                ..std::mem::zeroed()
            },
            ..std::mem::zeroed()
        };
        let mut pixels: *mut u8 = std::ptr::null_mut();
        let dib = CreateDIBSection(Some(dib_dc), &bi, DIB_RGB_COLORS, &mut pixels as *mut *mut u8 as _, None, 0)
            .expect("CreateDIBSection failed");
        let old_dib = SelectObject(dib_dc, dib.into());

        // 5. StretchBlt captured content into DIB (scaled up)
        SetStretchBltMode(dib_dc, HALFTONE);
        let _ = StretchBlt(dib_dc, 0, 0, d, d, Some(mem_dc), 0, 0, capture_w, capture_h, SRCCOPY);

        // 6. Circular clip — clear outside circle
        // Set alpha: 0 inside circle, 255 outside (for layered window transparency)
        let center = d as f64 / 2.0;
        let radius_sq = center * center;
        let pixel_slice = std::slice::from_raw_parts_mut(pixels, (d * d * 4) as usize);
        for row in 0..d {
            for col in 0..d {
                let dx = col as f64 - center + 0.5;
                let dy = row as f64 - center + 0.5;
                let dist_sq = dx * dx + dy * dy;
                let off = ((row * d + col) * 4) as usize;
                if dist_sq > radius_sq {
                    // Outside circle: transparent
                    pixel_slice[off] = 0;
                    pixel_slice[off + 1] = 0;
                    pixel_slice[off + 2] = 0;
                    pixel_slice[off + 3] = 0;
                } else if dist_sq > (center - BORDER_WIDTH as f64) * (center - BORDER_WIDTH as f64) {
                    // Border region: rainbow color
                    let (cr, cg, cb) = crate::overlay::color_at(col, row, 0, 0, d, d, color_mode, time_offset);
                    pixel_slice[off] = cb;     // B
                    pixel_slice[off + 1] = cg; // G
                    pixel_slice[off + 2] = cr; // R
                    pixel_slice[off + 3] = 255;
                }
                // else: keep the stretched content as-is (alpha already 255 from StretchBlt)
            }
        }

        // 7. Draw zoom text ("2.0x") at bottom center
        let zoom_text = format!("{:.1}x", zoom);
        SetBkMode(dib_dc, TRANSPARENT);
        // White text with black shadow for readability
        let font = CreateFontW(20, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, DEFAULT_CHARSET.0 as u32, 0, 0, CLEARTYPE_QUALITY, 0, windows::core::w!("Segoe UI"));
        let old_font = SelectObject(dib_dc, font.into());
        let text_y = d - 25;
        // Shadow
        SetTextColor(dib_dc, COLORREF(0x000000));
        let text_w = windows::core::w!("2.0x"); // measure roughly
        TextOutW(dib_dc, d / 2 - 15 + 1, text_y + 1, &windows::core::HSTRING::from(&zoom_text));
        // White text
        SetTextColor(dib_dc, COLORREF(0xFFFFFF));
        TextOutW(dib_dc, d / 2 - 15, text_y, &windows::core::HSTRING::from(&zoom_text));
        SelectObject(dib_dc, old_font);
        let _ = DeleteObject(font.into());

        // 8. UpdateLayeredWindow
        let mut ppt_dst = windows::Win32::Foundation::POINT { x, y };
        let mut size = windows::Win32::Foundation::SIZE { cx: d, cy: d };
        let mut ppt_src = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER.0,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA.0,
        };
        let _ = UpdateLayeredWindow(
            self.hwnd, Some(screen_dc), Some(&ppt_dst), Some(&size),
            Some(dib_dc), Some(&ppt_src), COLORREF(0), Some(&blend), ULW_ALPHA,
        );

        // 9. Cleanup
        SelectObject(dib_dc, old_dib);
        let _ = DeleteObject(dib.into());
        SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(cap_bmp.into());
        let _ = DeleteDC(dib_dc);
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(None, screen_dc);

        // 10. Show
        let _ = ShowWindow(self.hwnd, SW_SHOW);
    }
}
```

- [ ] **Step 4: Make color_at public in overlay.rs**

In `src/overlay.rs`, change `fn color_at(` to `pub(crate) fn color_at(`.

- [ ] **Step 5: Verify compilation**

Run: `cargo build 2>&1`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/magnifier.rs src/overlay.rs
git commit -m "feat(magnifier): implement window creation, screen capture, circular clip, rainbow border"
```

---

### Task 5: overlay.rs — Integration

**Files:**
- Modify: `src/overlay.rs`

**Interfaces:**
- Consumes: `MagnifierWindow::new()`, `MagnifierWindow::render()`, `MagnifierWindow::hide()` (from Task 3-4), `AppState.magnifier_active`, `AppState.zoom_level` (from Task 1)

- [ ] **Step 1: Add magnifier field to App struct**

```rust
pub struct App {
    // ... existing fields ...
    #[cfg(windows)]
    magnifier: Option<crate::magnifier::MagnifierWindow>,
}
```

- [ ] **Step 2: Initialize magnifier in App::new()**

In `App::new()`, add `magnifier: None` to the struct literal.

- [ ] **Step 3: Add magnifier rendering to render()**

At the end of `render()`, after `commit_dib(...)` and before the closing `}`:

```rust
// Magnifier rendering (after overlay commit)
if self.state.magnifier_active {
    let mag = self.magnifier.get_or_insert_with(|| {
        let overlay_hwnd = get_hwnd(window);
        crate::magnifier::MagnifierWindow::new(crate::magnifier::MAGNIFIER_DIAMETER, overlay_hwnd)
    });
    unsafe {
        let mut cursor_pos = windows::Win32::UI::Input::KeyboardAndMouse::POINT { x: 0, y: 0 };
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor_pos);
        mag.render((cursor_pos.x, cursor_pos.y), self.state.zoom_level, &self.color_mode, time_offset);
    }
} else if let Some(mag) = &self.magnifier {
    mag.hide();
}
```

- [ ] **Step 4: Add magnifier cleanup to Drop**

In `App::drop()`, add:

```rust
if let Some(mag) = &self.magnifier {
    mag.hide();
}
```

- [ ] **Step 5: Wire magnifier_active to decide_mouse**

In `mouse_hook_proc`, add a `MAGNIFIER_ACTIVE` AtomicBool (similar to `SHOULD_SUPPRESS`):

```rust
static MAGNIFIER_ACTIVE: AtomicBool = AtomicBool::new(false);
```

Pass `MAGNIFIER_ACTIVE.load(Ordering::Relaxed)` and `w_param.0 as isize` to `decide_mouse`.

Update the magnifier_active atomic when magnifier state changes:
- In the event processing loop (where `process_event` is called), after updating state, store `state.magnifier_active` into the atomic.

- [ ] **Step 6: Run full test suite**

Run: `cargo test --lib 2>&1`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add src/overlay.rs src/hook.rs
git commit -m "feat(overlay): integrate magnifier rendering into overlay event loop"
```

---

### Task 6: Verification + Polish

**Files:**
- All modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test --lib 2>&1`
Expected: ALL PASS

- [ ] **Step 2: Build release binary**

Run: `cargo build --release 2>&1`
Expected: PASS

- [ ] **Step 3: Manual smoke test**

Run the binary and verify:
1. Hold Alt+3 → magnifier appears at cursor, 2x zoom
2. Scroll up → zoom increases
3. Scroll down → zoom decreases
4. Release Alt → magnifier disappears
5. Hold Alt+3 again → zoom level preserved from last session
6. Alt+1 still works (pinned rects)
7. Alt+2 still works (spotlight)
8. Escape clears everything

- [ ] **Step 4: Commit any fixes**

```bash
git commit -m "fix: polish magnifier feature after manual testing"
```
