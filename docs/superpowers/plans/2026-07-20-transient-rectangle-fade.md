# Transient Rectangle Fade-Out Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fade completed non-pinned rectangle borders over 300 ms while preserving concurrent fades, rainbow motion, pinned content, and HoldRect's idle resource behavior.

**Architecture:** Keep `src/state.rs` unchanged. `overlay::App` owns a `Vec<FadingRect>` and updates it from pre-transition input state; the existing DIB pipeline redraws fading geometry each frame with premultiplied source-over alpha.

**Tech Stack:** Rust, winit event loop, Win32 GDI DIB, `UpdateLayeredWindow`, built-in `#[test]` tests.

## Global Constraints

- Follow strict red-green-refactor TDD; every production behavior must first have a test that fails for the expected reason.
- Run Cargo with one job: `cargo test -j 1`.
- Only transient borders fade; pinned rectangles and Spotlight masks do not fade.
- Duration is exactly 300 ms with `alpha = round(255 × (1 - p²))`.
- Mouse-up and modifier-release start fades; Escape clears active and ongoing fades immediately.
- Concurrent fades continue while new rectangles are drawn; rainbow motion continues.
- No new dependency, config option, thread, DIB snapshot, or `src/state.rs` change.
- Modify only `src/overlay.rs` plus this plan document.

---

### Task 1: Fade State, Event Capture, and Timing

**Files:**
- Modify/Test: `src/overlay.rs`

**Interfaces:**
- Produces: `FadingRect { rect: (i32, i32, i32, i32), started_at: Instant }`
- Produces: `fade_alpha(elapsed: Duration) -> u8`
- Produces: `update_fades_for_event(fades: &mut Vec<FadingRect>, state: &AppState, event: &InputEvent, now: Instant)`

- [ ] **Step 1: Write failing tests for the fade curve**

Add tests asserting exact endpoints, midpoint rounding, expiry, and monotonicity:

```rust
#[test]
fn fade_alpha_uses_300ms_quadratic_curve() {
    assert_eq!(fade_alpha(Duration::ZERO), 255);
    assert_eq!(fade_alpha(Duration::from_millis(150)), 191);
    assert_eq!(fade_alpha(Duration::from_millis(300)), 0);
    assert_eq!(fade_alpha(Duration::from_millis(500)), 0);
}

#[test]
fn fade_alpha_is_monotonic() {
    let samples = [0, 50, 100, 150, 200, 250, 300].map(|ms| {
        fade_alpha(Duration::from_millis(ms))
    });
    assert!(samples.windows(2).all(|pair| pair[0] >= pair[1]));
}
```

- [ ] **Step 2: Run the focused curve test and verify RED**

Run: `cargo test -j 1 overlay::tests::fade_alpha_uses_300ms_quadratic_curve -- --exact`

Expected: compile failure because `fade_alpha` does not exist.

- [ ] **Step 3: Implement the minimum curve and fade record**

```rust
use std::time::{Duration, Instant};

const FADE_DURATION: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq)]
struct FadingRect {
    rect: (i32, i32, i32, i32),
    started_at: Instant,
}

fn fade_alpha(elapsed: Duration) -> u8 {
    let p = (elapsed.as_secs_f32() / FADE_DURATION.as_secs_f32()).clamp(0.0, 1.0);
    (255.0 * (1.0 - p * p)).round() as u8
}
```

- [ ] **Step 4: Run both curve tests and verify GREEN**

Run: `cargo test -j 1 overlay::tests::fade_alpha`

Expected: both curve tests pass.

- [ ] **Step 5: Write failing tests for event capture**

Use a `DrawingState::Drawing` state and a fixed `Instant`. Cover:

```rust
#[test]
fn mouse_up_creates_transient_fade_at_release_position() {
    let now = Instant::now();
    let state = AppState {
        drawing: DrawingState::Drawing { start: (30, 40), current: (50, 60) },
        ..Default::default()
    };
    let mut fades = Vec::new();
    update_fades_for_event(
        &mut fades,
        &state,
        &InputEvent::MouseButtonUp { x: 10, y: 20 },
        now,
    );
    assert_eq!(fades, vec![FadingRect { rect: (10, 20, 30, 40), started_at: now }]);
}
```

Add separate tests proving modifier-release uses `current`, pinned completion does not fade, Escape clears existing fades, zero width/height is skipped, irrelevant states do nothing, and modifier-release followed by mouse-up creates only one fade when the second call receives the post-transition state.

- [ ] **Step 6: Run an event-capture test and verify RED**

Run: `cargo test -j 1 overlay::tests::mouse_up_creates_transient_fade_at_release_position -- --exact`

Expected: compile failure because `update_fades_for_event` does not exist.

- [ ] **Step 7: Implement event capture minimally**

```rust
fn update_fades_for_event(
    fades: &mut Vec<FadingRect>,
    state: &AppState,
    event: &InputEvent,
    now: Instant,
) {
    if matches!(event, InputEvent::EscapePressed) {
        fades.clear();
        return;
    }
    if state.pinned_active {
        return;
    }
    let DrawingState::Drawing { start, current } = &state.drawing else {
        return;
    };
    let end = match event {
        InputEvent::MouseButtonUp { x, y } => (*x, *y),
        InputEvent::ModifierChanged { pressed: false } => *current,
        _ => return,
    };
    let rect = normalize_rect(*start, end);
    if rect.0 < rect.2 && rect.1 < rect.3 {
        fades.push(FadingRect { rect, started_at: now });
    }
}
```

- [ ] **Step 8: Run all new timing/event tests and verify GREEN**

Run: `cargo test -j 1 overlay::tests::fade`

Expected: all fade-state tests pass.

- [ ] **Step 9: Commit the completed TDD slice**

```text
git add src/overlay.rs
git commit -m "feat: track transient rectangle fades"
```

---

### Task 2: Premultiplied Pixel Composition

**Files:**
- Modify/Test: `src/overlay.rs`

**Interfaces:**
- Changes: `fill_border_pixels(..., time_offset: f32, alpha: u8) -> (usize, usize)`
- Changes: `draw_rect_in_dib(..., time_offset: f32, alpha: u8)`
- Produces: fading writes use premultiplied source-over; alpha 255 retains the existing direct-write behavior.

- [ ] **Step 1: Write failing tests for translucent pixels**

Add tests that draw a solid red border with alpha 128:

```rust
#[test]
fn fading_border_premultiplies_color_over_transparent_pixel() {
    let mut buffer = vec![0u8; 4 * 4 * 4];
    fill_border_pixels(
        &mut buffer, 4, 4, 0, 0, (0, 0), (3, 3), 1,
        &ColorMode::Solid { r: 255, g: 0, b: 0 }, 0.0, 128,
    );
    assert_eq!(&buffer[0..4], &[0, 0, 128, 128]);
}
```

Add separate tests for source-over onto a black Spotlight pixel `[0, 0, 0, 160]`, opaque pinned/active overwrite after a fade, and two fading writes in oldest-to-newest order.

- [ ] **Step 2: Run the premultiplication test and verify RED**

Run: `cargo test -j 1 overlay::tests::fading_border_premultiplies_color_over_transparent_pixel -- --exact`

Expected: compile failure because `fill_border_pixels` has no alpha parameter.

- [ ] **Step 3: Implement the minimum source-over write**

Extend `fill_border_pixels` with `alpha: u8`. In its pixel closure:

```rust
if alpha == 0 {
    return false;
}
let was_zero = buf[offset + 3] == 0;
if alpha == 255 {
    buf[offset..offset + 4].copy_from_slice(&[b, g, r, 255]);
} else {
    let alpha = alpha as u32;
    let inv = 255 - alpha;
    let blend = |src: u8, dst: u8| {
        ((src as u32 * alpha + dst as u32 * inv + 127) / 255) as u8
    };
    buf[offset] = blend(b, buf[offset]);
    buf[offset + 1] = blend(g, buf[offset + 1]);
    buf[offset + 2] = blend(r, buf[offset + 2]);
    buf[offset + 3] = (alpha
        + (buf[offset + 3] as u32 * inv + 127) / 255) as u8;
}
was_zero
```

The RGB formula premultiplies the source and performs source-over in one expression because existing destination channels are already premultiplied. Pass `255` at every existing opaque call site and propagate alpha through `draw_rect_in_dib`.

- [ ] **Step 4: Run pixel tests and verify GREEN**

Run: `cargo test -j 1 overlay::tests::fading_border`

Expected: all new fading pixel tests pass.

- [ ] **Step 5: Run existing border tests for regression coverage**

Run: `cargo test -j 1 overlay::tests::fill_border`

Expected: all existing geometry, clipping, rainbow, duplicate-write, and pixel-count tests pass unchanged after receiving alpha `255`.

- [ ] **Step 6: Commit the completed TDD slice**

```text
git add src/overlay.rs
git commit -m "feat: blend fading rectangle borders"
```

---

### Task 3: Event Loop, Rendering, and Overlay Lifecycle

**Files:**
- Modify/Test: `src/overlay.rs`

**Interfaces:**
- Changes: `App` owns `fading_rects: Vec<FadingRect>`.
- Changes: `should_show_overlay(has_drawing: bool, has_pinned: bool, has_fades: bool) -> bool`.
- Rendering order: Spotlight, fading oldest-to-newest, pinned, active.

- [ ] **Step 1: Write failing visibility/lifecycle tests**

Update existing `should_show_overlay` tests and `topmost_enforce_count` calls for the third argument (`false` for frames without fades), then add:

```rust
#[test]
fn overlay_shown_when_only_fading() {
    assert!(should_show_overlay(false, false, true));
}
```

Add a deterministic expiry test using two `FadingRect` entries and a fixed `now`, retaining only entries where `now.saturating_duration_since(started_at) < FADE_DURATION`.

- [ ] **Step 2: Run the fade-only visibility test and verify RED**

Run: `cargo test -j 1 overlay::tests::overlay_shown_when_only_fading -- --exact`

Expected: compile failure because `should_show_overlay` accepts only two arguments.

- [ ] **Step 3: Integrate fade state into `App` and per-event processing**

Add `fading_rects: Vec<FadingRect>` to `App`, initialize it empty, and call before `process_event` for every drained event:

```rust
let now = Instant::now();
update_fades_for_event(&mut self.fading_rects, &self.state, &event, now);
let new_state = process_event(&self.state, &event);
```

- [ ] **Step 4: Integrate cleanup, visibility, rendering, and scheduling**

At the start of `render`, capture one `now`, remove expired fades, and include fade presence in `should_show_overlay`. Render in this order:

```rust
// existing Spotlight mask
for fade in &self.fading_rects {
    draw_rect_in_dib(
        cache, width, height, wr.left, wr.top,
        (fade.rect.0, fade.rect.1), (fade.rect.2, fade.rect.3),
        self.border_width, &self.color_mode, time_offset,
        fade_alpha(now.saturating_duration_since(fade.started_at)),
    );
}
// existing pinned loop with alpha 255
// existing active drawing with alpha 255
```

Change visibility to `should_show_overlay(has_drawing, has_pinned, has_fades)`. Reuse it in `about_to_wait` so fades keep the 16 ms loop alive:

```rust
let has_drawing = matches!(self.state.drawing, DrawingState::Drawing { .. });
let has_pinned = !self.state.pinned_rects.is_empty();
let has_fades = !self.fading_rects.is_empty();
let needs_animation = should_show_overlay(has_drawing, has_pinned, has_fades)
    || self.state.magnifier_active
    || self.popup_manager.needs_frame();
```

- [ ] **Step 5: Run lifecycle tests and verify GREEN**

Run: `cargo test -j 1 overlay::tests::overlay_`

Expected: drawing, pinned, combined, fade-only, and hidden cases pass.

- [ ] **Step 6: Run the complete test suite**

Run: `cargo test -j 1`

Expected: all tests pass with no failures or warnings.

- [ ] **Step 7: Run formatting and compile verification**

Run: `cargo fmt -- --check`

Expected: exit code 0.

Run: `cargo build -j 1`

Expected: build succeeds.

- [ ] **Step 8: Commit the integration**

```text
git add src/overlay.rs
git commit -m "feat: fade transient rectangles after release"
```

---

### Task 4: Review and Fix

**Files:**
- Review: `src/overlay.rs`
- Update if required: `src/overlay.rs`, `docs/MISTAKE.md`

- [ ] **Step 1: Review the complete diff against the design**

Check event ordering, MouseButtonUp final coordinates, modifier-release behavior, Escape clearing, pinned exclusion, zero-area exclusion, concurrent fades, render order, alpha math, rainbow flow, expiration-before-hide, and 16 ms scheduling.

- [ ] **Step 2: For every discovered bug, add a failing regression test first**

Run the smallest matching test and confirm RED before changing production code.

- [ ] **Step 3: Apply the minimal fix and verify GREEN**

Run the focused regression test, then `cargo test -j 1`.

- [ ] **Step 4: Record only recurring tooling/API mistakes**

If a recurring error was encountered, append a concise prevention rule to `docs/MISTAKE.md`. Do not add feature notes or one-off mistakes.

- [ ] **Step 5: Final verification**

Run:

```text
cargo fmt -- --check
cargo test -j 1
cargo build -j 1
git diff --check
git status --short
```

Expected: formatting clean, all tests pass, build succeeds, no whitespace errors, and only intended files are modified.

- [ ] **Step 6: Commit review fixes if any**

```text
git add src/overlay.rs docs/MISTAKE.md
git commit -m "fix: address transient fade review findings"
```
