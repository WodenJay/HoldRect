# Pinned Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Pinned mode — digit key `1` toggles pin, pinned rects freeze on screen after mouse release, multiple pinned rects coexist, Esc clears all.

**Architecture:** Extend the existing state machine (`state.rs`) with `pinned_rects` and `pinned_active`. Add `DigitPressed`/`EscapePressed` events from hook. Refactor overlay DIB rendering to support multiple rects in a single frame.

**Tech Stack:** Rust, Win32 GDI (UpdateLayeredWindow), winit event loop.

## Global Constraints

- TDD: write failing test first, then implement, then verify pass
- `cargo test` concurrency = 1 (memory constraint)
- No mock/dead code, no `#[allow(dead_code)]`
- Existing tests must keep passing after each task
- `process_event` remains a pure function (no side effects)

---

### Task 1: Extend InputEvent with DigitPressed and EscapePressed

**Files:**
- Modify: `src/state.rs:1-8` (InputEvent enum)
- Test: `src/state.rs` (same file, #[cfg(test)] module)

**Interfaces:**
- Produces: `InputEvent::DigitPressed(u8)`, `InputEvent::EscapePressed` — used by Tasks 2, 3, 4

- [ ] **Step 1: Add new variants to InputEvent**

In `src/state.rs`, add two variants to the `InputEvent` enum:

```rust
/// Input events from the global listener
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    DigitPressed(u8),
    EscapePressed,
}
```

- [ ] **Step 2: Verify existing tests still compile**

Run: `cargo test -p holdrect --lib state -- --quiet`
Expected: all existing tests PASS (new variants unused but compile fine)

- [ ] **Step 3: Commit**

```bash
git add src/state.rs
git commit -m "feat(state): add DigitPressed and EscapePressed to InputEvent"
```

---

### Task 2: Extend AppState with pinned_rects and pinned_active

**Files:**
- Modify: `src/state.rs:20-33` (AppState struct + Default impl)
- Modify: `src/state.rs` tests (all 26 `AppState { ... }` literals need `..Default::default()`)

**Interfaces:**
- Produces: `AppState { drawing, pinned_rects: Vec<(i32,i32,i32,i32)>, pinned_active: bool }` — used by Tasks 3, 4, 5

- [ ] **Step 1: Write failing test for new AppState fields**

In `src/state.rs` `#[cfg(test)]` module, add:

```rust
#[test]
fn default_app_state_has_empty_pinned() {
    let state = AppState::default();
    assert!(state.pinned_rects.is_empty());
    assert!(!state.pinned_active);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p holdrect --lib state::tests::default_app_state_has_empty_pinned -- --quiet`
Expected: FAIL — `pinned_rects` field not found

- [ ] **Step 3: Add fields to AppState and Default**

Replace the `AppState` struct and its `Default` impl:

```rust
/// Application state
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<(i32, i32, i32, i32)>,
    pub pinned_active: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            drawing: DrawingState::Idle,
            pinned_rects: Vec::new(),
            pinned_active: false,
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p holdrect --lib state::tests::default_app_state_has_empty_pinned -- --quiet`
Expected: PASS

- [ ] **Step 5: Update all existing test AppState literals**

Every `AppState { drawing: ... }` in tests needs `..Default::default()` appended. Replace all occurrences. For example:

```rust
// Before:
let state = AppState { drawing: DrawingState::Idle };
// After:
let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
```

This applies to all test functions in `src/state.rs`. There are 26 occurrences of `AppState {` and 1 of `AppState::default()` in tests. The `AppState::default()` usage needs no change.

For the multi-line `AppState` literals like:
```rust
let state = AppState {
    drawing: DrawingState::Drawing { start: (10, 20), current: (10, 20) },
};
```
Change to:
```rust
let state = AppState {
    drawing: DrawingState::Drawing { start: (10, 20), current: (10, 20) },
    ..Default::default()
};
```

- [ ] **Step 6: Run ALL state tests to verify nothing broke**

Run: `cargo test -p holdrect --lib state -- --quiet`
Expected: all tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/state.rs
git commit -m "feat(state): add pinned_rects and pinned_active to AppState"
```

---

### Task 3: Implement pinned state transitions in process_event

**Files:**
- Modify: `src/state.rs:36-66` (process_event function)
- Modify: `src/state.rs` tests (new tests for pinned transitions)

**Interfaces:**
- Consumes: `AppState { drawing, pinned_rects, pinned_active }`, `InputEvent::DigitPressed`, `InputEvent::EscapePressed`
- Produces: Updated `process_event` behavior — Tasks 4, 5 depend on these state transitions

- [ ] **Step 1: Write failing tests for pinned transitions**

In `src/state.rs` `#[cfg(test)]` module, add these tests:

```rust
// --- Pinned mode: DigitPressed toggle ---

#[test]
fn armed_digit_1_toggles_pinned_active() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(1));
    assert!(next.pinned_active);
    assert_eq!(next.drawing, DrawingState::Armed);
}

#[test]
fn armed_digit_1_toggle_off() {
    let state = AppState { drawing: DrawingState::Armed, pinned_active: true, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(1));
    assert!(!next.pinned_active);
}

#[test]
fn drawing_digit_1_toggles_pinned_active() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::DigitPressed(1));
    assert!(next.pinned_active);
    assert_eq!(next.drawing, DrawingState::Drawing { start: (10, 20), current: (50, 80) });
}

#[test]
fn idle_digit_1_is_noop() {
    let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(1));
    assert!(!next.pinned_active);
    assert_eq!(next.drawing, DrawingState::Idle);
}

#[test]
fn digit_other_than_1_is_ignored() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(2));
    assert!(!next.pinned_active);
}

// --- Pinned mode: mouse up with pinned_active ---

#[test]
fn drawing_mouse_up_pinned_pushes_rect() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
    assert_eq!(next.drawing, DrawingState::Armed);
    assert_eq!(next.pinned_rects, vec![(10, 20, 50, 80)]);
    assert!(!next.pinned_active, "pinned_active resets after mouse up");
}

#[test]
fn drawing_mouse_up_pinned_normalizes_rect() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (50, 80), current: (10, 20) },
        pinned_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::MouseButtonUp { x: 10, y: 20 });
    assert_eq!(next.pinned_rects, vec![(10, 20, 50, 80)]);
}

#[test]
fn drawing_mouse_up_not_pinned_clears_rect() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_active: false,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
    assert_eq!(next.drawing, DrawingState::Armed);
    assert!(next.pinned_rects.is_empty());
}

// --- Pinned mode: multiple rects accumulate ---

#[test]
fn multiple_pinned_rects_accumulate() {
    let mut state = AppState::default();
    // First rect: modifier → toggle → draw → mouse up
    state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
    state = process_event(&state, &InputEvent::DigitPressed(1));
    state = process_event(&state, &InputEvent::MouseButtonDown { x: 10, y: 10 });
    state = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 50 });
    assert_eq!(state.pinned_rects.len(), 1);
    assert_eq!(state.pinned_rects[0], (10, 10, 50, 50));

    // Second rect: still modifier held, draw another (pinned_active reset, need to toggle again)
    state = process_event(&state, &InputEvent::DigitPressed(1));
    state = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 100 });
    state = process_event(&state, &InputEvent::MouseButtonUp { x: 200, y: 200 });
    assert_eq!(state.pinned_rects.len(), 2);
    assert_eq!(state.pinned_rects[1], (100, 100, 200, 200));
}

// --- Pinned mode: per-rect reset ---

#[test]
fn pinned_active_resets_after_mouse_up() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
    assert!(!next.pinned_active);
}

// --- EscapePressed ---

#[test]
fn escape_clears_pinned_rects() {
    let state = AppState {
        drawing: DrawingState::Armed,
        pinned_rects: vec![(10, 20, 50, 80), (100, 100, 200, 200)],
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::EscapePressed);
    assert!(next.pinned_rects.is_empty());
    assert_eq!(next.drawing, DrawingState::Armed);
}

#[test]
fn escape_during_draw_cancels_and_clears_pinned() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_rects: vec![(0, 0, 100, 100)],
        pinned_active: true,
    };
    let next = process_event(&state, &InputEvent::EscapePressed);
    assert_eq!(next.drawing, DrawingState::Armed);
    assert!(next.pinned_rects.is_empty());
    assert!(!next.pinned_active);
}

#[test]
fn escape_in_idle_clears_pinned_rects() {
    let state = AppState {
        drawing: DrawingState::Idle,
        pinned_rects: vec![(10, 20, 50, 80)],
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::EscapePressed);
    assert!(next.pinned_rects.is_empty());
    assert_eq!(next.drawing, DrawingState::Idle);
}

// --- Modifier release resets pinned_active ---

#[test]
fn modifier_release_resets_pinned_active() {
    let state = AppState {
        drawing: DrawingState::Armed,
        pinned_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
    assert!(!next.pinned_active);
    assert_eq!(next.drawing, DrawingState::Idle);
}

#[test]
fn drawing_modifier_release_with_pinned_pushes_rect() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
    assert_eq!(next.pinned_rects, vec![(10, 20, 50, 80)]);
    assert!(!next.pinned_active);
    assert_eq!(next.drawing, DrawingState::Idle);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p holdrect --lib state::tests::armed_digit_1 -- --quiet`
Expected: FAIL — `DigitPressed` not matched in process_event

- [ ] **Step 3: Implement new process_event logic**

Replace the `process_event` function entirely:

```rust
/// Pure state transition function. No side effects.
pub fn process_event(state: &AppState, event: &InputEvent) -> AppState {
    let (drawing, pinned_active, pinned_rects) = match (&state.drawing, event) {
        // --- DigitPressed(1) toggle (only in Armed or Drawing, i.e. modifier held) ---
        (DrawingState::Armed, InputEvent::DigitPressed(1)) => {
            (state.drawing.clone(), !state.pinned_active, state.pinned_rects.clone())
        }
        (DrawingState::Drawing { .. }, InputEvent::DigitPressed(1)) => {
            (state.drawing.clone(), !state.pinned_active, state.pinned_rects.clone())
        }

        // --- EscapePressed: clear all pinned rects ---
        (_, InputEvent::EscapePressed) => {
            let drawing = match &state.drawing {
                DrawingState::Drawing { .. } => DrawingState::Armed,
                other => other.clone(),
            };
            (drawing, false, Vec::new())
        }

        // --- Existing transitions (pinned_active/pinned_rects unchanged unless noted) ---

        // Idle -> Armed on modifier press
        (DrawingState::Idle, InputEvent::ModifierChanged { pressed: true }) => {
            (DrawingState::Armed, state.pinned_active, state.pinned_rects.clone())
        }
        // Armed -> Drawing on mouse down
        (DrawingState::Armed, InputEvent::MouseButtonDown { x, y }) => {
            (DrawingState::Drawing { start: (*x, *y), current: (*x, *y) }, state.pinned_active, state.pinned_rects.clone())
        }
        // Drawing: update current position on mouse move
        (DrawingState::Drawing { start, .. }, InputEvent::MouseMove { x, y }) => {
            (DrawingState::Drawing { start: *start, current: (*x, *y) }, state.pinned_active, state.pinned_rects.clone())
        }
        // Drawing -> Armed on mouse up
        (DrawingState::Drawing { start, current }, InputEvent::MouseButtonUp { .. }) => {
            let mut rects = state.pinned_rects.clone();
            if state.pinned_active {
                let (x0, y0, x1, y1) = normalize_rect(*start, *current);
                rects.push((x0, y0, x1, y1));
            }
            (DrawingState::Armed, false, rects)
        }
        // Armed -> Idle on modifier release
        (DrawingState::Armed, InputEvent::ModifierChanged { pressed: false }) => {
            (DrawingState::Idle, false, state.pinned_rects.clone())
        }
        // Drawing -> Idle on modifier release
        (DrawingState::Drawing { start, current }, InputEvent::ModifierChanged { pressed: false }) => {
            let mut rects = state.pinned_rects.clone();
            if state.pinned_active {
                let (x0, y0, x1, y1) = normalize_rect(*start, *current);
                rects.push((x0, y0, x1, y1));
            }
            (DrawingState::Idle, false, rects)
        }
        // All other combinations: no state change
        _ => (state.drawing.clone(), state.pinned_active, state.pinned_rects.clone()),
    };
    AppState { drawing, pinned_rects, pinned_active }
}
```

Note: this uses `normalize_rect` which is currently defined in `overlay.rs`. Move it to `state.rs` (or make it `pub(crate)` and import). The simplest approach: copy `normalize_rect` into `state.rs` as a `fn normalize_rect(...)` since it's a pure 4-line function.

```rust
fn normalize_rect(start: (i32, i32), current: (i32, i32)) -> (i32, i32, i32, i32) {
    let x0 = start.0.min(current.0);
    let y0 = start.1.min(current.1);
    let x1 = start.0.max(current.0);
    let y1 = start.1.max(current.1);
    (x0, y0, x1, y1)
}
```

Place it above `process_event`, outside `#[cfg(test)]`.

- [ ] **Step 4: Run ALL state tests**

Run: `cargo test -p holdrect --lib state -- --quiet`
Expected: all tests PASS (old + new)

- [ ] **Step 5: Commit**

```bash
git add src/state.rs
git commit -m "feat(state): implement pinned mode transitions with Esc and per-rect reset"
```

---

### Task 4: Extend hook to emit DigitPressed and EscapePressed

**Files:**
- Modify: `src/hook.rs:117-123` (decide_keyboard function)
- Modify: `src/hook.rs:59` (call site in keyboard_hook_proc)
- Test: `src/hook.rs` tests

**Interfaces:**
- Consumes: `InputEvent::DigitPressed(1)`, `InputEvent::EscapePressed` from Task 1
- Produces: hook emits new events — consumed by overlay via channel

- [ ] **Step 1: Write failing tests for digit and Esc handling**

In `src/hook.rs` `#[cfg(test)]` module, add:

```rust
// --- DigitPressed and EscapePressed tests ---

#[test]
fn digit_1_modifier_held_emits_digit_pressed() {
    let result = decide_keyboard(0x31, true, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, Some(InputEvent::DigitPressed(1)));
}

#[test]
fn digit_1_modifier_not_held_returns_none() {
    let result = decide_keyboard(0x31, true, &[0x12, 0xA4, 0xA5], false);
    assert_eq!(result, None);
}

#[test]
fn digit_1_key_up_returns_none() {
    let result = decide_keyboard(0x31, false, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, None);
}

#[test]
fn digit_2_modifier_held_returns_none() {
    // Only digit 1 handled for now; 2 is reserved for Spotlight
    let result = decide_keyboard(0x32, true, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, None);
}

#[test]
fn escape_modifier_held_emits_escape_pressed() {
    let result = decide_keyboard(0x1B, true, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, Some(InputEvent::EscapePressed));
}

#[test]
fn escape_modifier_not_held_also_emits_escape_pressed() {
    // Esc works without modifier — user can clear pinned rects anytime
    let result = decide_keyboard(0x1B, true, &[0x12, 0xA4, 0xA5], false);
    assert_eq!(result, Some(InputEvent::EscapePressed));
}

#[test]
fn escape_key_up_returns_none() {
    let result = decide_keyboard(0x1B, false, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p holdrect --lib hook::tests::digit_1_modifier_held -- --quiet`
Expected: FAIL — signature mismatch (3 params vs 4)

- [ ] **Step 3: Update decide_keyboard signature and logic**

Replace the `decide_keyboard` function:

```rust
// Pure decision function — no Win32 side effects, fully unit-testable
pub(crate) fn decide_keyboard(vk_code: u32, is_key_down: bool, modifier_codes: &[u32], modifier_held: bool) -> Option<InputEvent> {
    if modifier_codes.contains(&vk_code) {
        return Some(InputEvent::ModifierChanged { pressed: is_key_down });
    }
    if is_key_down {
        if modifier_held && vk_code == 0x31 {
            return Some(InputEvent::DigitPressed(1));
        }
        if vk_code == 0x1B {
            return Some(InputEvent::EscapePressed);
        }
    }
    None
}
```

- [ ] **Step 4: Update call site in keyboard_hook_proc**

In `src/hook.rs`, find the call at line 59:

```rust
if let Some(event) = decide_keyboard(kb.vkCode, is_key_down, MODIFIER_CODES.get().expect("MODIFIER_CODES not set")) {
```

Replace with:

```rust
let modifier_held = SHOULD_SUPPRESS.load(Ordering::Relaxed);
if let Some(event) = decide_keyboard(kb.vkCode, is_key_down, MODIFIER_CODES.get().expect("MODIFIER_CODES not set"), modifier_held) {
```

- [ ] **Step 5: Update ALL existing decide_keyboard test calls**

Every existing `decide_keyboard(vk, down, codes)` call in tests needs a 4th arg. Most modifier tests should pass `false` for modifier_held (they're testing modifier detection, not digit/Esc). Pattern:

```rust
// Before:
let result = decide_keyboard(VK_LMENU.0 as u32, true, &[0x12, 0xA4, 0xA5]);
// After:
let result = decide_keyboard(VK_LMENU.0 as u32, true, &[0x12, 0xA4, 0xA5], false);
```

For the integration-style tests that simulate a full session (where `should_suppress` would be true during modifier hold), pass `true` for modifier_held when appropriate. Check each test's intent.

- [ ] **Step 6: Run ALL hook tests**

Run: `cargo test -p holdrect --lib hook -- --quiet`
Expected: all tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/hook.rs
git commit -m "feat(hook): emit DigitPressed(1) and EscapePressed when modifier held"
```

---

### Task 5: Refactor overlay DIB rendering for multi-rect support

**Files:**
- Modify: `src/overlay.rs:349-462` (draw_border function)
- Modify: `src/overlay.rs:117-231` (App struct, about_to_wait, render)

**Interfaces:**
- Consumes: `AppState.pinned_rects`, `AppState.drawing` from Tasks 2-3
- Produces: `clear_dib`, `draw_rect_in_dib`, `commit_dib` — internal to overlay.rs

This task refactors `draw_border` into three functions and updates `render()` and `about_to_wait` to support multi-rect rendering. No new external interfaces.

- [ ] **Step 1: Extract clear_dib from draw_border**

In `src/overlay.rs`, the `draw_border` function currently does three things: (1) allocates/resizes DIB, (2) clears to transparent + draws pixels, (3) calls UpdateLayeredWindow. Split step 1+2's "clear" part into `clear_dib`.

Add these functions above the existing `draw_border`:

```rust
/// Ensure DIB is allocated to the correct size.
#[cfg(windows)]
fn ensure_dib_size(dib_cache: &mut Option<DibCache>, width: i32, height: i32) {
    match dib_cache {
        Some(cache) => cache.ensure_size(width, height),
        None => {
            *dib_cache = DibCache::new(width, height);
        }
    }
}

/// Clear the DIB to fully transparent pixels.
#[cfg(windows)]
fn clear_dib_pixels(cache: &mut DibCache, width: i32, height: i32) {
    unsafe {
        std::ptr::write_bytes(cache.pixels, 0, width as usize * height as usize * 4);
    }
}
```

- [ ] **Step 2: Extract draw_rect_in_dib**

Add this function. It draws one rect's border pixels into an existing (already-cleared) DIB:

```rust
/// Draw one rectangle's border into the DIB buffer. Does NOT call UpdateLayeredWindow.
#[cfg(windows)]
fn draw_rect_in_dib(
    cache: &mut DibCache,
    width: i32,
    height: i32,
    win_x: i32,
    win_y: i32,
    start: (i32, i32),
    current: (i32, i32),
    border_width: i32,
    color_mode: &ColorMode,
    time_offset: f32,
) {
    unsafe {
        let pixel_slice = std::slice::from_raw_parts_mut(
            cache.pixels,
            width as usize * height as usize * 4,
        );
        fill_border_pixels(
            pixel_slice, width, height, win_x, win_y,
            start, current, border_width, color_mode, time_offset,
        );
    }
}
```

- [ ] **Step 3: Extract commit_dib**

Add this function. It calls UpdateLayeredWindow to push the DIB to screen:

```rust
/// Push the DIB buffer to screen via UpdateLayeredWindow.
#[cfg(windows)]
fn commit_dib(window: &Window, cache: &DibCache, width: i32, height: i32, win_x: i32, win_y: i32) {
    use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let destination = POINT { x: win_x, y: win_y };
        let source = POINT { x: 0, y: 0 };
        let size = SIZE { cx: width, cy: height };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let screen_dc = GetDC(HWND::default());
        let result = UpdateLayeredWindow(
            hwnd, screen_dc, Some(&destination), Some(&size),
            cache.memory_dc, Some(&source), COLORREF(0), Some(&blend), ULW_ALPHA,
        );
        if let Err(error) = result {
            eprintln!("UpdateLayeredWindow failed: {error:?}");
        }
        let _ = ReleaseDC(HWND::default(), screen_dc);
    }
}
```

- [ ] **Step 4: Rewrite render() to support multi-rect**

Replace the `render` method in the `impl App` block (the one at line 220):

```rust
fn render(&mut self) {
    let Some(window) = &self.window else { return; };

    let has_drawing = matches!(&self.state.drawing, DrawingState::Drawing { .. });
    let has_pinned = !self.state.pinned_rects.is_empty();

    if !has_drawing && !has_pinned {
        window.set_visible(false);
        #[cfg(windows)]
        hide_from_alt_tab(window);
        return;
    }

    #[cfg(windows)]
    {
        let hwnd = get_hwnd(window);
        let mut wr = windows::Win32::Foundation::RECT::default();
        if unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wr) }.is_err() {
            return;
        }
        let width = wr.right - wr.left;
        let height = wr.bottom - wr.top;
        if width <= 0 || height <= 0 { return; }

        ensure_dib_size(&mut self.dib_cache, width, height);
        let cache = match &mut self.dib_cache {
            Some(c) if !c.pixels.is_null() => c,
            _ => return,
        };
        clear_dib_pixels(cache, width, height);

        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let time_offset = (elapsed.as_secs_f64() * FLOW_SPEED as f64).fract() as f32;

        // Draw all pinned rects
        for &(x0, y0, x1, y1) in &self.state.pinned_rects {
            draw_rect_in_dib(cache, width, height, wr.left, wr.top,
                             (x0, y0), (x1, y1), self.border_width, &self.color_mode, time_offset);
        }

        // Draw active rect on top
        if let DrawingState::Drawing { start, current } = &self.state.drawing {
            draw_rect_in_dib(cache, width, height, wr.left, wr.top,
                             *start, *current, self.border_width, &self.color_mode, time_offset);
        }

        show_window_topmost(window);
        commit_dib(window, cache, width, height, wr.left, wr.top);
    }
}
```

- [ ] **Step 5: Rewrite about_to_wait**

Replace the `about_to_wait` method:

```rust
fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    // Drain all pending input events
    while let Ok(event) = self.input_rx.try_recv() {
        let new_state = process_event(&self.state, &event);
        self.state = new_state;
    }

    self.render();

    let needs_animation = matches!(&self.state.drawing, DrawingState::Drawing { .. })
        || !self.state.pinned_rects.is_empty();

    if needs_animation {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    } else {
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}
```

- [ ] **Step 6: Remove old draw_border function**

Delete the old `draw_border` function (lines 353-462) since its logic is now split across `ensure_dib`, `clear_dib_pixels`, `draw_rect_in_dib`, and `commit_dib`.

- [ ] **Step 7: Build to verify compilation**

Run: `cargo build -p holdrect`
Expected: compiles without errors

- [ ] **Step 8: Run ALL tests**

Run: `cargo test -p holdrect -- --quiet`
Expected: all tests PASS

- [ ] **Step 9: Commit**

```bash
git add src/overlay.rs
git commit -m "feat(overlay): refactor DIB rendering for multi-rect pinned support"
```

---

### Task 6: End-to-end manual verification

**Files:** None (manual testing)

- [ ] **Step 1: Build release binary**

Run: `cargo build --release -p holdrect`
Expected: compiles successfully

- [ ] **Step 2: Run the app and test pinned mode**

Run: `target/release/holdrect.exe`

Test scenarios:
1. Hold Alt + drag → normal transient rect (disappears on release) ✓
2. Hold Alt + press `1` + drag + release → rect stays frozen ✓
3. Hold Alt + press `1` again (toggle off) + drag + release → rect disappears ✓
4. Draw multiple pinned rects → all visible simultaneously ✓
5. Press Esc → all pinned rects cleared ✓
6. Hold Alt + drag + press Esc mid-draw → current draw cancelled ✓
7. Rainbow animation flows on pinned rects ✓

- [ ] **Step 3: Commit any fixes if needed**

If manual testing reveals issues, fix and commit.

```bash
git add -A
git commit -m "fix: address issues from manual pinned mode verification"
```
