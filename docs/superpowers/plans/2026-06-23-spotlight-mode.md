# Spotlight Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Spotlight mode — digit 2 toggle dims the area outside the drawn rect, per-rect, combinable with pinned.

**Architecture:** Introduce `PinnedRect` struct replacing tuple, add `spotlight_active` to state machine, add `dim_outside_spotlights` rendering function that writes directly to the DIB pixel buffer.

**Tech Stack:** Rust, Win32 GDI (DIB section), winit

**Spec:** `docs/superpowers/specs/2026-06-23-spotlight-mode-design.md`

## Global Constraints

- `cargo test` max concurrency = 1 (memory constraint)
- TDD: write failing test first, then implement
- Each task ends with independently passing tests + commit
- No mockup/dead code

---

### Task 1: Add PinnedRect struct and update AppState + process_event + overlay

**Files:**
- Modify: `src/state.rs` — add `PinnedRect` struct, update `AppState`, update `process_event` return type and all match arms, update all tests
- Modify: `src/overlay.rs` — update `render()` to use struct fields instead of tuple destructuring (lines 249-251), update tests that construct pinned_rects with tuples

**Interfaces:**
- Produces: `pub struct PinnedRect { pub x0: i32, pub y0: i32, pub x1: i32, pub y1: i32, pub spotlight: bool }`
- Produces: `pub struct AppState { pub drawing: DrawingState, pub pinned_rects: Vec<PinnedRect>, pub pinned_active: bool }` (spotlight_active added in Task 3)
- Produces: `process_event(state: &AppState, event: &InputEvent) -> AppState` (return type changes from 3-tuple to struct fields)

- [ ] **Step 1: Write PinnedRect struct in state.rs**

Add before `AppState` (after `DrawingState` enum, around line 22):

```rust
/// A pinned rectangle with per-rect flags
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub spotlight: bool,
}
```

- [ ] **Step 2: Update AppState to use Vec\<PinnedRect\>**

Change line 27 from `pub pinned_rects: Vec<(i32, i32, i32, i32)>` to `pub pinned_rects: Vec<PinnedRect>`.

Default impl stays the same (`Vec::new()` works for both).

- [ ] **Step 3: Run tests to see what breaks**

Run: `cargo test -j 1 --lib 2>&1 | head -80`
Expected: Many compile errors in `state.rs` tests (tuple literals) and `overlay.rs` (tuple destructuring).

- [ ] **Step 4: Update process_event to use PinnedRect**

In `process_event` (line 50-109), update the match arms that push to `pinned_rects`:

Line 88-89 (MouseUp with pinned):
```rust
// Before:
let (x0, y0, x1, y1) = normalize_rect(*start, final_current);
rects.push((x0, y0, x1, y1));

// After:
let (x0, y0, x1, y1) = normalize_rect(*start, final_current);
rects.push(PinnedRect { x0, y0, x1, y1, spotlight: false });
```

Line 100-102 (modifier release with pinned):
```rust
// Before:
let (x0, y0, x1, y1) = normalize_rect(*start, *current);
rects.push((x0, y0, x1, y1));

// After:
let (x0, y0, x1, y1) = normalize_rect(*start, *current);
rects.push(PinnedRect { x0, y0, x1, y1, spotlight: false });
```

- [ ] **Step 5: Update all state.rs tests to use PinnedRect**

Replace every tuple literal in pinned_rects with PinnedRect struct literal — both in `pinned_rects: vec![...]` constructions AND in `assert_eq!` comparisons. Examples:

```rust
// Construction — before:
pinned_rects: vec![(10, 20, 50, 80)]
// Construction — after:
pinned_rects: vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: false }]

// Assertion — before:
assert_eq!(state.pinned_rects[0], (10, 10, 50, 50));
// Assertion — after:
assert_eq!(state.pinned_rects[0], PinnedRect { x0: 10, y0: 10, x1: 50, y1: 50, spotlight: false });
```

Add `use super::PinnedRect;` or `use crate::state::PinnedRect;` to test imports.

Affected tests (search for `pinned_rects` and tuple assertions in tests):
- `drawing_mouse_up_pinned_pushes_rect` (line 451) — construction + assertion
- `drawing_mouse_up_pinned_normalizes_rect` (line 464) — construction + assertion
- `multiple_pinned_rects_accumulate` (line 489) — constructions + **assert_eq! on [0] and [1]**
- `escape_clears_pinned_rects` (line 523) — construction
- `escape_during_draw_cancels_and_clears_pinned` (line 535) — construction
- `escape_in_idle_clears_pinned_rects` (line 548) — construction
- `drawing_modifier_release_with_pinned_pushes_rect` (line 574) — construction + assertion

- [ ] **Step 6: Update overlay.rs render() to use struct fields**

Line 249: change `for &(x0, y0, x1, y1) in &self.state.pinned_rects` to:
```rust
for rect in &self.state.pinned_rects {
    draw_rect_in_dib(cache, width, height, wr.left, wr.top,
                     (rect.x0, rect.y0), (rect.x1, rect.y1), self.border_width, &self.color_mode, time_offset);
}
```

- [ ] **Step 7: Update overlay.rs tests that construct pinned_rects**

Search overlay.rs test module for tuple literals in `pinned_rects`. Update to use `PinnedRect`. Also add `use crate::state::PinnedRect;` to the test imports.

- [ ] **Step 8: Run all tests**

Run: `cargo test -j 1 --lib 2>&1 | tail -20`
Expected: All tests pass (same behavior, just struct instead of tuple).

- [ ] **Step 9: Commit**

```bash
git add src/state.rs src/overlay.rs
git commit -m "refactor: replace pinned_rects tuple with PinnedRect struct"
```

---

### Task 2: Add digit 2 hook support

**Files:**
- Modify: `src/hook.rs` — add `vk_code == 0x32` branch in `decide_keyboard`, update existing test, add new tests

**Interfaces:**
- Consumes: `InputEvent::DigitPressed(u8)` (already generic)
- Produces: `decide_keyboard` emits `DigitPressed(2)` when `modifier_held && vk_code == 0x32`

- [ ] **Step 1: Write failing test for digit 2**

Add in `src/hook.rs` test module (after existing digit 1 tests around line 573):

```rust
#[test]
fn digit_2_modifier_held_emits_digit_pressed_2() {
    let result = decide_keyboard(0x32, true, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, Some(InputEvent::DigitPressed(2)));
}

#[test]
fn digit_2_modifier_not_held_returns_none() {
    let result = decide_keyboard(0x32, true, &[0x12, 0xA4, 0xA5], false);
    assert_eq!(result, None);
}

#[test]
fn digit_2_key_up_returns_none() {
    let result = decide_keyboard(0x32, false, &[0x12, 0xA4, 0xA5], true);
    assert_eq!(result, None);
}
```

- [ ] **Step 2: Run tests to verify digit 2 tests fail**

Run: `cargo test -j 1 --lib digit_2 2>&1 | tail -20`
Expected: FAIL — `digit_2_modifier_held_emits_digit_pressed_2` fails (returns None instead of DigitPressed(2)).

- [ ] **Step 3: Update existing test — REMOVE it**

The existing test `digit_2_modifier_held_returns_none` at hook.rs:~585 is now covered by the new test in Step 1. Remove it to avoid duplicate assertions:

```rust
// DELETE this test entirely:
// #[test]
// fn digit_2_modifier_held_returns_none() { ... }
```

- [ ] **Step 4: Implement digit 2 in decide_keyboard**

In `decide_keyboard` (line 122-135), add after the digit 1 check (line 128-129):

```rust
if modifier_held && vk_code == 0x31 {
    return Some(InputEvent::DigitPressed(1));
}
if modifier_held && vk_code == 0x32 {
    return Some(InputEvent::DigitPressed(2));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -j 1 --lib decide_keyboard 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src/hook.rs
git commit -m "feat(hook): emit DigitPressed(2) for digit key 2"
```

---

### Task 3: Add spotlight_active to state machine

**Files:**
- Modify: `src/state.rs` — add `spotlight_active` field to `AppState`, expand `process_event` to 4-tuple, add all spotlight state tests

**Interfaces:**
- Consumes: `InputEvent::DigitPressed(2)` (from Task 2)
- Consumes: `PinnedRect` struct (from Task 1)
- Produces: `AppState.spotlight_active: bool`
- Produces: `PinnedRect.spotlight` is set from `spotlight_active` on commit

- [ ] **Step 1: Write failing tests for spotlight state transitions**

Add at end of `state.rs` test module (before the closing `}`):

```rust
// --- Spotlight mode: DigitPressed(2) toggle ---

#[test]
fn armed_digit_2_toggles_spotlight_active() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(2));
    assert!(next.spotlight_active);
    assert_eq!(next.drawing, DrawingState::Armed);
}

#[test]
fn armed_digit_2_toggle_off() {
    let state = AppState { drawing: DrawingState::Armed, spotlight_active: true, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(2));
    assert!(!next.spotlight_active);
}

#[test]
fn drawing_digit_2_toggles_spotlight_active() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::DigitPressed(2));
    assert!(next.spotlight_active);
    assert_eq!(next.drawing, DrawingState::Drawing { start: (10, 20), current: (50, 80) });
}

#[test]
fn idle_digit_2_is_noop() {
    let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(2));
    assert!(!next.spotlight_active);
    assert_eq!(next.drawing, DrawingState::Idle);
}

// --- Spotlight mode: mouse up with spotlight ---

#[test]
fn drawing_mouse_up_pinned_spotlight_pushes_rect_with_spotlight_true() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_active: true,
        spotlight_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
    assert_eq!(next.pinned_rects.len(), 1);
    assert!(next.pinned_rects[0].spotlight);
    assert!(!next.spotlight_active, "spotlight_active resets after mouse up");
}

#[test]
fn drawing_mouse_up_pinned_no_spotlight_pushes_spotlight_false() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_active: true,
        spotlight_active: false,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
    assert_eq!(next.pinned_rects.len(), 1);
    assert!(!next.pinned_rects[0].spotlight);
}

#[test]
fn spotlight_active_resets_after_mouse_up() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        spotlight_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
    assert!(!next.spotlight_active);
}

// --- Spotlight + Pinned independence ---

#[test]
fn pinned_and_spotlight_independent() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(1));
    let next = process_event(&next, &InputEvent::DigitPressed(2));
    assert!(next.pinned_active);
    assert!(next.spotlight_active);
}

#[test]
fn digit_1_does_not_affect_spotlight() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(1));
    assert!(!next.spotlight_active);
}

#[test]
fn digit_2_does_not_affect_pinned() {
    let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
    let next = process_event(&state, &InputEvent::DigitPressed(2));
    assert!(!next.pinned_active);
}

// --- Spotlight: EscapePressed ---

#[test]
fn escape_resets_spotlight_active() {
    let state = AppState {
        drawing: DrawingState::Armed,
        spotlight_active: true,
        pinned_rects: vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: true }],
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::EscapePressed);
    assert!(!next.spotlight_active);
    assert!(next.pinned_rects.is_empty());
}

// --- Spotlight: modifier release ---

#[test]
fn modifier_release_resets_spotlight_active() {
    let state = AppState {
        drawing: DrawingState::Armed,
        spotlight_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
    assert!(!next.spotlight_active);
    assert_eq!(next.drawing, DrawingState::Idle);
}

#[test]
fn drawing_modifier_release_with_pinned_spotlight_pushes_rect() {
    let state = AppState {
        drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        pinned_active: true,
        spotlight_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
    assert_eq!(next.pinned_rects.len(), 1);
    assert!(next.pinned_rects[0].spotlight);
    assert!(!next.spotlight_active);
    assert_eq!(next.drawing, DrawingState::Idle);
}

// --- Spotlight: multiple rects accumulate ---

#[test]
fn multiple_spotlight_rects_accumulate() {
    let mut state = AppState::default();
    // First spotlight rect
    state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
    state = process_event(&state, &InputEvent::DigitPressed(2));
    state = process_event(&state, &InputEvent::DigitPressed(1)); // also pinned
    state = process_event(&state, &InputEvent::MouseButtonDown { x: 10, y: 10 });
    state = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 50 });
    assert_eq!(state.pinned_rects.len(), 1);
    assert!(state.pinned_rects[0].spotlight);

    // Second spotlight rect (must re-toggle)
    state = process_event(&state, &InputEvent::DigitPressed(2));
    state = process_event(&state, &InputEvent::DigitPressed(1));
    state = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 100 });
    state = process_event(&state, &InputEvent::MouseButtonUp { x: 200, y: 200 });
    assert_eq!(state.pinned_rects.len(), 2);
    assert!(state.pinned_rects[1].spotlight);
}

// --- Mixed spotlight and non-spotlight ---

#[test]
fn mixed_spotlight_and_non_spotlight_rects() {
    let mut state = AppState::default();
    // Non-spotlight pinned rect
    state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
    state = process_event(&state, &InputEvent::DigitPressed(1));
    state = process_event(&state, &InputEvent::MouseButtonDown { x: 10, y: 10 });
    state = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 50 });
    assert!(!state.pinned_rects[0].spotlight);

    // Spotlight pinned rect
    state = process_event(&state, &InputEvent::DigitPressed(1));
    state = process_event(&state, &InputEvent::DigitPressed(2));
    state = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 100 });
    state = process_event(&state, &InputEvent::MouseButtonUp { x: 200, y: 200 });
    assert!(!state.pinned_rects[0].spotlight, "first rect unchanged");
    assert!(state.pinned_rects[1].spotlight, "second rect is spotlight");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -j 1 --lib armed_digit_2 2>&1 | tail -10`
Expected: FAIL — `spotlight_active` field not found.

- [ ] **Step 3: Add spotlight_active to AppState**

In `AppState` struct (line 25-29), add field:
```rust
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<PinnedRect>,
    pub pinned_active: bool,
    pub spotlight_active: bool,
}
```

Update `Default` impl (line 31-38):
```rust
impl Default for AppState {
    fn default() -> Self {
        Self {
            drawing: DrawingState::Idle,
            pinned_rects: Vec::new(),
            pinned_active: false,
            spotlight_active: false,
        }
    }
}
```

- [ ] **Step 4: Expand process_event match to include spotlight_active**

The `process_event` function (line 50) destructures into a 3-tuple. Change every match arm to also carry `spotlight_active`. The cleanest approach: change the local binding from:
```rust
let (drawing, pinned_active, pinned_rects) = match (&state.drawing, event) {
```
to:
```rust
let (drawing, pinned_active, spotlight_active, pinned_rects) = match (&state.drawing, event) {
```

Then update EVERY arm to include `state.spotlight_active` (or the toggled value). Key changes:

DigitPressed(1) arms — spotlight unchanged:
```rust
(DrawingState::Armed, InputEvent::DigitPressed(1)) => {
    (state.drawing.clone(), !state.pinned_active, state.spotlight_active, state.pinned_rects.clone())
}
(DrawingState::Drawing { .. }, InputEvent::DigitPressed(1)) => {
    (state.drawing.clone(), !state.pinned_active, state.spotlight_active, state.pinned_rects.clone())
}
```

NEW DigitPressed(2) arms:
```rust
(DrawingState::Armed, InputEvent::DigitPressed(2)) => {
    (state.drawing.clone(), state.pinned_active, !state.spotlight_active, state.pinned_rects.clone())
}
(DrawingState::Drawing { .. }, InputEvent::DigitPressed(2)) => {
    (state.drawing.clone(), state.pinned_active, !state.spotlight_active, state.pinned_rects.clone())
}
```

EscapePressed — reset both flags:
```rust
(_, InputEvent::EscapePressed) => {
    let drawing = match &state.drawing {
        DrawingState::Drawing { .. } => DrawingState::Armed,
        other => other.clone(),
    };
    (drawing, false, false, Vec::new())
}
```

All other arms — pass through `state.spotlight_active` unchanged (add as 3rd element).

MouseUp with pinned — set spotlight from spotlight_active:
```rust
(DrawingState::Drawing { start, .. }, InputEvent::MouseButtonUp { x, y }) => {
    let final_current = (*x, *y);
    let mut rects = state.pinned_rects.clone();
    if state.pinned_active {
        let (x0, y0, x1, y1) = normalize_rect(*start, final_current);
        rects.push(PinnedRect { x0, y0, x1, y1, spotlight: state.spotlight_active });
    }
    (DrawingState::Armed, false, false, rects)
}
```

Modifier release with pinned:
```rust
(DrawingState::Drawing { start, current }, InputEvent::ModifierChanged { pressed: false }) => {
    let mut rects = state.pinned_rects.clone();
    if state.pinned_active {
        let (x0, y0, x1, y1) = normalize_rect(*start, *current);
        rects.push(PinnedRect { x0, y0, x1, y1, spotlight: state.spotlight_active });
    }
    (DrawingState::Idle, false, false, rects)
}
```

Modifier release without pinned:
```rust
(DrawingState::Armed, InputEvent::ModifierChanged { pressed: false }) => {
    (DrawingState::Idle, false, false, state.pinned_rects.clone())
}
```

Default arm:
```rust
_ => (state.drawing.clone(), state.pinned_active, state.spotlight_active, state.pinned_rects.clone()),
```

Final return:
```rust
AppState { drawing, pinned_rects, pinned_active, spotlight_active }
```

- [ ] **Step 5: Run all tests**

Run: `cargo test -j 1 --lib 2>&1 | tail -20`
Expected: All pass (old + new).

- [ ] **Step 6: Commit**

```bash
git add src/state.rs
git commit -m "feat(state): add spotlight_active toggle with per-rect reset"
```

---

### Task 4: Add spotlight rendering (dim_outside_spotlights)

**Files:**
- Modify: `src/overlay.rs` — add `dim_outside_spotlights` function, update `render()` to call it, add render tests

**Interfaces:**
- Consumes: `AppState.spotlight_active` (from Task 3)
- Consumes: `PinnedRect.spotlight` (from Task 1)
- Consumes: `DibCache.pixels` (raw pointer to BGRA buffer)
- Produces: `fn dim_outside_spotlights(buffer: &mut [u8], width: i32, height: i32, rects: &[(i32, i32, i32, i32)], window_left: i32, window_top: i32)`
  - `rects` param takes pre-filtered spotlight rects as (x0,y0,x1,y1) in screen coords
  - Wrapper `dim_outside_spotlights_in_dib(cache: &mut DibCache, ...)` handles raw pointer → slice conversion (same pattern as `draw_rect_in_dib`)

- [ ] **Step 1: Write failing render test for dim_outside_spotlights**

Add in overlay.rs test module (inside `mod missing_tests` or new `mod spotlight_tests`):

```rust
mod spotlight_tests {
    use super::super::dim_outside_spotlights;
    use super::super::DibCache;

    #[test]
    fn dim_outside_spotlights_fills_dark_outside_rect() {
        // Create a small 10x10 DIB-like buffer
        let width = 10i32;
        let height = 10i32;
        let mut buf = vec![0u8; (width * height * 4) as usize];

        // Spotlight rect at (2,2)-(7,7)
        let rects = vec![(2, 2, 7, 7)];
        dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

        // Pixel outside rect should be dimmed (alpha=160)
        let outside_offset = (0 * width as usize + 0) * 4;
        assert_eq!(buf[outside_offset + 3], 160, "outside pixel alpha should be 160");
        assert_eq!(buf[outside_offset], 0, "B=0");
        assert_eq!(buf[outside_offset + 1], 0, "G=0");
        assert_eq!(buf[outside_offset + 2], 0, "R=0");
    }

    #[test]
    fn dim_outside_spotlights_clears_interior() {
        let width = 10i32;
        let height = 10i32;
        let mut buf = vec![0u8; (width * height * 4) as usize];

        let rects = vec![(2, 2, 7, 7)];
        dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

        // Pixel inside rect should be cleared (alpha=0)
        let inside_offset = (4 * width as usize + 4) * 4;
        assert_eq!(buf[inside_offset + 3], 0, "inside pixel alpha should be 0");
    }

    #[test]
    fn dim_outside_spotlights_noop_when_empty() {
        let width = 10i32;
        let height = 10i32;
        let mut buf = vec![0u8; (width * height * 4) as usize];

        dim_outside_spotlights(&mut buf, width, height, &[], 0, 0);

        // All pixels should stay 0
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn dim_outside_spotlights_mixed_spotlight_and_non_spotlight() {
        let width = 20i32;
        let height = 20i32;
        let mut buf = vec![0u8; (width * height * 4) as usize];

        // Only one spotlight rect; non-spotlight rects are not passed to dim function
        let rects = vec![(5, 5, 10, 10)]; // spotlight rect
        dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

        // Inside spotlight rect should be clear
        let inside_offset = (7 * width as usize + 7) * 4;
        assert_eq!(buf[inside_offset + 3], 0, "spotlight interior should be clear");

        // Outside spotlight rect should be dimmed (even where a non-spotlight rect would be)
        let outside_offset = (0 * width as usize + 0) * 4;
        assert_eq!(buf[outside_offset + 3], 160, "outside spotlight should be dimmed");

        // Inside a non-spotlight rect area (e.g. 15,15) is also dimmed — only spotlight rects get cleared
        let non_spotlight_interior = (15 * width as usize + 15) * 4;
        assert_eq!(buf[non_spotlight_interior + 3], 160, "non-spotlight interior stays dimmed");
    }

    #[test]
    fn dim_outside_spotlights_overlapping_rects() {
        let width = 20i32;
        let height = 20i32;
        let mut buf = vec![0u8; (width * height * 4) as usize];

        // Two overlapping spotlight rects
        let rects = vec![(2, 2, 10, 10), (5, 5, 15, 15)];
        dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

        // Pixel in overlap region should be cleared
        let overlap_offset = (7 * width as usize + 7) * 4;
        assert_eq!(buf[overlap_offset + 3], 0, "overlap interior should be clear");

        // Pixel outside both should be dimmed
        let outside_offset = (0 * width as usize + 0) * 4;
        assert_eq!(buf[outside_offset + 3], 160, "outside both should be dimmed");
    }

    #[test]
    fn dim_outside_spotlights_with_window_offset() {
        let width = 10i32;
        let height = 10i32;
        let mut buf = vec![0u8; (width * height * 4) as usize];

        // Rect in screen coords (12, 12) to (17, 17), window at (10, 10)
        // Local coords: (2, 2) to (7, 7)
        let rects = vec![(12, 12, 17, 17)];
        dim_outside_spotlights(&mut buf, width, height, &rects, 10, 10);

        // Inside (local 4,4) should be clear
        let inside_offset = (4 * width as usize + 4) * 4;
        assert_eq!(buf[inside_offset + 3], 0, "inside should be clear with offset");

        // Outside (local 0,0) should be dimmed
        let outside_offset = 0;
        assert_eq!(buf[outside_offset + 3], 160, "outside should be dimmed with offset");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -j 1 --lib dim_outside_spotlights 2>&1 | tail -10`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement dim_outside_spotlights**

Add after `clear_dib_pixels` (around line 393) in overlay.rs:

```rust
/// Dim all pixels outside the given spotlight rects.
/// `rects` are (x0, y0, x1, y1) in screen coordinates.
/// Interior of each rect (x0..=x1, y0..=y1) is cleared to transparent.
fn dim_outside_spotlights(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    rects: &[(i32, i32, i32, i32)],
    win_x: i32,
    win_y: i32,
) {
    if rects.is_empty() || width <= 0 || height <= 0 {
        return;
    }

    let stride = width as usize * 4;
    let total_pixels = width as usize * height as usize;

    // Dim all pixels: BGRA = (0, 0, 0, 160)
    for i in 0..total_pixels {
        let off = i * 4;
        buffer[off] = 0;     // B
        buffer[off + 1] = 0; // G
        buffer[off + 2] = 0; // R
        buffer[off + 3] = 160; // A (semi-transparent)
    }

    // Clear interior of each spotlight rect to fully transparent
    for &(sx0, sy0, sx1, sy1) in rects {
        let x0 = (sx0 - win_x).clamp(0, width - 1);
        let y0 = (sy0 - win_y).clamp(0, height - 1);
        let x1 = (sx1 - win_x).clamp(0, width - 1);
        let y1 = (sy1 - win_y).clamp(0, height - 1);

        if x1 < x0 || y1 < y0 {
            continue;
        }

        for y in y0..=y1 {
            for x in x0..=x1 {
                let off = y as usize * stride + x as usize * 4;
                buffer[off] = 0;
                buffer[off + 1] = 0;
                buffer[off + 2] = 0;
                buffer[off + 3] = 0;
            }
        }
    }
}
```

- [ ] **Step 4: Run spotlight render tests**

Run: `cargo test -j 1 --lib dim_outside_spotlights 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 5: Add DIB wrapper for dim_outside_spotlights**

Add after the pure `dim_outside_spotlights` function:

```rust
/// Wrapper that operates directly on DibCache (same pattern as draw_rect_in_dib).
fn dim_outside_spotlights_in_dib(
    cache: &mut DibCache,
    width: i32,
    height: i32,
    rects: &[(i32, i32, i32, i32)],
    win_x: i32,
    win_y: i32,
) {
    unsafe {
        let pixel_slice = std::slice::from_raw_parts_mut(
            cache.pixels,
            width as usize * height as usize * 4,
        );
        dim_outside_spotlights(pixel_slice, width, height, rects, win_x, win_y);
    }
}
```

- [ ] **Step 6: Update render() to call dim_outside_spotlights_in_dib**

In `render()` (line 212), add `use crate::state::normalize_rect;` to the imports at top of overlay.rs if not present. Then after `clear_dib_pixels` (line 241) and before the pinned_rects loop (line 249), add:

```rust
// Build spotlight rects list (screen coords)
let mut spotlight_rects: Vec<(i32, i32, i32, i32)> = self.state.pinned_rects.iter()
    .filter(|r| r.spotlight)
    .map(|r| (r.x0, r.y0, r.x1, r.y1))
    .collect();

// Include active drawing rect if spotlight_active
if self.state.spotlight_active {
    if let DrawingState::Drawing { start, current } = &self.state.drawing {
        let (x0, y0, x1, y1) = normalize_rect(*start, *current);
        spotlight_rects.push((x0, y0, x1, y1));
    }
}

// Dim outside spotlight rects
dim_outside_spotlights_in_dib(cache, width, height, &spotlight_rects, wr.left, wr.top);
```

- [ ] **Step 7: Update pinned_rects loop to use struct fields**

Line 249 should already be updated from Task 1. If not, update:
```rust
for rect in &self.state.pinned_rects {
    draw_rect_in_dib(cache, width, height, wr.left, wr.top,
                     (rect.x0, rect.y0), (rect.x1, rect.y1), self.border_width, &self.color_mode, time_offset);
}
```

- [ ] **Step 8: Run all tests**

Run: `cargo test -j 1 --lib 2>&1 | tail -20`
Expected: All pass.

- [ ] **Step 9: Commit**

```bash
git add src/overlay.rs
git commit -m "feat(overlay): add spotlight dim rendering outside rect"
```

---

### Task 5: Build and manual smoke test

**Files:** None modified

- [ ] **Step 1: Full build**

Run: `cargo build 2>&1 | tail -10`
Expected: Compiles with no errors.

- [ ] **Step 2: Full test suite**

Run: `cargo test -j 1 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 3: Run the application manually**

Run: `cargo run` (or the built exe)

Test:
1. Hold Alt, drag to draw a rect → should show rainbow border (existing behavior)
2. Hold Alt, press 2, drag to draw a rect → area outside rect should dim, inside stays transparent
3. Release mouse → if pinned (1), rect stays; if spotlight (2), dim stays with rect
4. Press Esc → all rects and dim cleared
5. Hold Alt, press 1 then 2, drag → pinned + spotlight rect

- [ ] **Step 4: Commit any fixes if needed**

If manual testing found issues, fix and commit.
