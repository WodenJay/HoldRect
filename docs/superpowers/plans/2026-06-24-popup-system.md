# Popup System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add status popup (digit key toggle feedback) and cheatsheet popup (modifier+` hold) to HoldRect with Apple-style spring animation and GDI rendering.

**Architecture:** PopupManager (pure state machine) drives animation phases. GdiRenderer handles Win32 GDI drawing (rounded rects, text, shadow). Raw `CreateWindowExW` popup window with `UpdateLayeredWindow` compositing. All integrated into existing winit event loop via `about_to_wait`.

**Tech Stack:** Rust, Win32 GDI (`CreateCompatibleDC`, `RoundRect`, `DrawTextW`, `UpdateLayeredWindow`), existing `windows` crate features.

## Global Constraints

- No new crate dependencies — all APIs in existing `windows` crate features
- `Cargo.toml` feature flags already cover all required Win32 APIs
- Popup window uses `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW`
- Compositing via `UpdateLayeredWindow` + `ULW_ALPHA` (per-pixel alpha)
- TDD: write failing test first for every non-trivial logic change
- Commit after each task
- No icons/emojis in popup text (user decision overriding PRD)
- Hold duration = 1000ms (overrides PRD's 0.5s)
- `GdiRenderer` must implement `Drop` for GDI resource cleanup

## File Structure

```
src/popup/
    mod.rs          — PopupManager, PopupContent, PopupPhase, build_status_text()
    animation.rs    — spring_position() pure function
    gdi_renderer.rs — GdiRenderer struct, GDI drawing (#[cfg(windows)])

> **ponytail: `renderer.rs` / `PopupRenderer` trait deferred.** Spec calls for it now, but YAGNI — only one renderer exists. Add trait when v0.3 actually adds a second platform. Breaking refactor at that point is ~10 min of extracting methods into a trait.

Modified files:
    src/state.rs    — Add ToggleHelp, HideHelp to InputEvent enum
    src/hook.rs     — Add VK_OEM_3 detection in decide_keyboard
    src/config.rs   — Add modifier_name field to AppConfig
    src/overlay.rs  — Add popup fields to App, wire up in about_to_wait
```

---

### Task 1: Spring Animation (pure function)

**Files:**
- Create: `src/popup/animation.rs`
- Create: `src/popup/mod.rs` (module declaration only)

**Interfaces:**
- Produces: `spring_position(elapsed_secs: f64, start: f64, target: f64, omega_n: f64, zeta: f64) -> f64`

- [ ] **Step 1: Create popup module with animation**

Create `src/popup/mod.rs`:
```rust
pub mod animation;
```

Create `src/popup/animation.rs`:
```rust
/// Critically-damped (or underdamped) spring position.
/// Returns the value at time `elapsed_secs` given spring parameters.
/// At t=0 returns `start`. As t->inf returns `target`.
pub fn spring_position(elapsed_secs: f64, start: f64, target: f64, omega_n: f64, zeta: f64) -> f64 {
    // stub — tests will drive the implementation
    start
}
```

Add `pub mod popup;` to `src/main.rs` (after existing module declarations).

- [ ] **Step 2: Write failing tests for spring_position**

Add to `src/popup/animation.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::spring_position;

    const EPSILON: f64 = 0.5;

    #[test]
    fn at_t0_returns_start() {
        let pos = spring_position(0.0, -60.0, 0.0, 18.0, 0.82);
        assert!((pos - (-60.0)).abs() < EPSILON);
    }

    #[test]
    fn converges_to_target() {
        let pos = spring_position(10.0, -60.0, 0.0, 18.0, 0.82);
        assert!((pos - 0.0).abs() < EPSILON);
    }

    #[test]
    fn underdamped_overshoots() {
        // zeta < 1 should overshoot target
        let mut max_pos = f64::MIN;
        for i in 0..500 {
            let t = i as f64 * 0.001;
            let pos = spring_position(t, -60.0, 0.0, 18.0, 0.82);
            if pos > max_pos {
                max_pos = pos;
            }
        }
        assert!(max_pos > 0.0, "underdamped spring should overshoot past target, got max={}", max_pos);
    }

    #[test]
    fn critically_damped_no_overshoot() {
        // zeta = 1.0 should never exceed target
        let mut max_pos = f64::MIN;
        for i in 0..1000 {
            let t = i as f64 * 0.001;
            let pos = spring_position(t, -60.0, 0.0, 22.0, 1.0);
            if pos > max_pos {
                max_pos = pos;
            }
        }
        assert!(max_pos <= 0.5, "critically damped should not overshoot, got max={}", max_pos);
    }

    #[test]
    fn converges_within_500ms() {
        let pos = spring_position(0.5, -60.0, 0.0, 18.0, 0.82);
        assert!((pos - 0.0).abs() < 2.0, "should be near target at 500ms, got {}", pos);
    }

    #[test]
    fn overshoot_is_roughly_4_percent() {
        // start=-60, target=0, displacement=60. 4% overshoot = ~2.4px past 0
        let mut max_pos = f64::MIN;
        for i in 0..500 {
            let t = i as f64 * 0.001;
            let pos = spring_position(t, -60.0, 0.0, 18.0, 0.82);
            if pos > max_pos {
                max_pos = pos;
            }
        }
        // Allow 2-5% range
        assert!(max_pos > 1.0 && max_pos < 5.0,
            "overshoot should be ~2-4px, got {}", max_pos);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib popup::animation -- 2>&1 | head -40`
Expected: FAIL (stub returns `start` always, `converges_to_target` fails)

- [ ] **Step 4: Implement spring_position**

Replace the stub in `src/popup/animation.rs`:
```rust
/// Critically-damped (or underdamped) spring position.
/// Returns the value at time `elapsed_secs` given spring parameters.
/// At t=0 returns `start`. As t->inf returns `target`.
pub fn spring_position(elapsed_secs: f64, start: f64, target: f64, omega_n: f64, zeta: f64) -> f64 {
    let displacement = start - target;
    let t = elapsed_secs;

    if t <= 0.0 {
        return start;
    }

    if zeta >= 1.0 {
        // Critically damped or overdamped
        // x(t) = target + displacement * (1 + omega_n * t) * exp(-omega_n * t)
        let decay = (-omega_n * t).exp();
        target + displacement * (1.0 + omega_n * t) * decay
    } else {
        // Underdamped
        // x(t) = target + displacement * exp(-zeta * omega_n * t) *
        //         (cos(omega_d * t) + (zeta / sqrt(1 - zeta^2)) * sin(omega_d * t))
        let omega_d = omega_n * (1.0 - zeta * zeta).sqrt();
        let decay = (-zeta * omega_n * t).exp();
        let cos_part = (omega_d * t).cos();
        let sin_part = (zeta / (1.0 - zeta * zeta).sqrt()) * (omega_d * t).sin();
        target + displacement * decay * (cos_part + sin_part)
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib popup::animation -- 2>&1 | head -20`
Expected: all 6 PASS

- [ ] **Step 6: Commit**

```bash
git add src/popup/mod.rs src/popup/animation.rs src/main.rs
git commit -m "feat(popup): add spring animation pure function with tests"
```

---

### Task 2: InputEvent + VK_OEM_3 Detection

**Files:**
- Modify: `src/state.rs:3-10` (InputEvent enum)
- Modify: `src/hook.rs:122-138` (decide_keyboard)
- Modify: `src/hook.rs` (decide_keyboard tests section)

**Interfaces:**
- Produces: `InputEvent::ToggleHelp`, `InputEvent::HideHelp` variants
- Consumes: existing `decide_keyboard` signature unchanged

- [ ] **Step 1: Add ToggleHelp and HideHelp to InputEvent**

In `src/state.rs`, add two variants to the `InputEvent` enum:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    DigitPressed(u8),
    EscapePressed,
    ToggleHelp,   // modifier + ` pressed
    HideHelp,     // modifier or ` released
}
```

- [ ] **Step 2: Write failing tests for VK_OEM_3 detection**

In the `decide_keyboard` tests section of `src/hook.rs` (find the test module that contains `decide_keyboard_*` tests), add:
```rust
#[test]
fn modifier_held_backtick_down_emits_toggle_help() {
    let alt_codes: &[u32] = &[0x12, 0xA4, 0xA5];
    let event = decide_keyboard(0xC0, true, alt_codes, true); // VK_OEM_3 down, modifier held
    assert_eq!(event, Some(InputEvent::ToggleHelp));
}

#[test]
fn backtick_down_no_modifier_is_none() {
    let alt_codes: &[u32] = &[0x12, 0xA4, 0xA5];
    let event = decide_keyboard(0xC0, true, alt_codes, false); // VK_OEM_3 down, no modifier
    assert_eq!(event, None);
}

#[test]
fn backtick_up_emits_hide_help() {
    let alt_codes: &[u32] = &[0x12, 0xA4, 0xA5];
    let event = decide_keyboard(0xC0, false, alt_codes, true);
    assert_eq!(event, Some(InputEvent::HideHelp));
}

#[test]
fn modifier_up_while_backtick_held_emits_hide_help() {
    let alt_codes: &[u32] = &[0x12, 0xA4, 0xA5];
    // Modifier key release (0x12 = VK_LMENU)
    let event = decide_keyboard(0x12, false, alt_codes, true);
    // This should be ModifierChanged, not HideHelp — modifier release is handled by ModifierChanged
    assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: false }));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib decide_keyboard -- 2>&1 | head -30`
Expected: `modifier_held_backtick_down_emits_toggle_help` FAILS (returns None)

- [ ] **Step 4: Implement VK_OEM_3 detection in decide_keyboard**

In `src/hook.rs`, add backtick handling after the digit key checks, before the Escape check:
```rust
pub(crate) fn decide_keyboard(vk_code: u32, is_key_down: bool, modifier_codes: &[u32], modifier_held: bool) -> Option<InputEvent> {
    if modifier_codes.contains(&vk_code) {
        return Some(InputEvent::ModifierChanged { pressed: is_key_down });
    }
    if is_key_down {
        if modifier_held && vk_code == 0x31 {
            return Some(InputEvent::DigitPressed(1));
        }
        if modifier_held && vk_code == 0x32 {
            return Some(InputEvent::DigitPressed(2));
        }
        if modifier_held && vk_code == 0xC0 {
            return Some(InputEvent::ToggleHelp);
        }
        if vk_code == 0x1B {
            return Some(InputEvent::EscapePressed);
        }
    }
    // Backtick release (any state) — hide cheatsheet
    if !is_key_down && vk_code == 0xC0 {
        return Some(InputEvent::HideHelp);
    }
    None
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib decide_keyboard -- 2>&1 | head -20`
Expected: all PASS (new tests + all existing decide_keyboard tests)

- [ ] **Step 6: Commit**

```bash
git add src/state.rs src/hook.rs src/overlay.rs
git commit -m "feat(hook): add ToggleHelp/HideHelp for VK_OEM_3 detection"
```

---

### Task 3: PopupManager State Machine

**Files:**
- Modify: `src/popup/mod.rs`
- Modify: `src/popup/animation.rs` (make constants public)

**Interfaces:**
- Produces: `PopupManager` with methods `on_event()`, `show_status()`, `show_cheatsheet()`, `hide_cheatsheet()`, `tick()`, `needs_frame()`, `is_visible()`, `current_y_offset()`, `status_text()`, `content()`
- Produces: `build_status_text(pinned: bool, spotlight: bool) -> String`

- [ ] **Step 1: Define PopupContent, PopupPhase, PopupManager structs**

Replace `src/popup/mod.rs`:
```rust
pub mod animation;

use std::time::Instant;
use crate::state::{AppState, InputEvent};

const SLIDE_IN_DURATION_MS: u64 = 400;
const HOLD_DURATION_MS: u64 = 1000;
const SLIDE_OUT_DURATION_MS: u64 = 300;
const START_Y_OFFSET: f64 = -60.0;
const TARGET_Y_OFFSET: f64 = 0.0;

// Spring params: slide-in (slightly underdamped)
const SLIDE_IN_OMEGA: f64 = 18.0;
const SLIDE_IN_ZETA: f64 = 0.82;

// Spring params: slide-out (critically damped)
const SLIDE_OUT_OMEGA: f64 = 22.0;
const SLIDE_OUT_ZETA: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub enum PopupContent {
    Status,
    Cheatsheet,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PopupPhase {
    Hidden,
    SlidingIn { started_at: Instant },
    Holding { started_at: Instant },
    SlidingOut { started_at: Instant, from_y: f64 },
}

pub fn build_status_text(pinned: bool, spotlight: bool) -> String {
    match (pinned, spotlight) {
        (false, false) => "Transient".to_string(),
        (true, false) => "Pinned".to_string(),
        (false, true) => "Spotlight".to_string(),
        (true, true) => "Pinned \u{00b7} Spotlight".to_string(),
    }
}

pub struct PopupManager {
    content: PopupContent,
    phase: PopupPhase,
    status_text: String,
    cheatsheet_rows: Vec<(String, String)>,
}

impl PopupManager {
    pub fn new(modifier_name: &str) -> Self {
        let drag_label = format!("{} + drag", modifier_name);
        let help_label = format!("{} + `", modifier_name);
        Self {
            content: PopupContent::Status,
            phase: PopupPhase::Hidden,
            status_text: String::new(),
            cheatsheet_rows: vec![
                (drag_label, "Draw".to_string()),
                ("1".to_string(), "Pin".to_string()),
                ("2".to_string(), "Spotlight".to_string()),
                ("Esc".to_string(), "Clear".to_string()),
                (help_label, "Help".to_string()),
            ],
        }
    }

    pub fn on_event(&mut self, event: &InputEvent, state: &AppState) {
        match event {
            InputEvent::DigitPressed(1) | InputEvent::DigitPressed(2) => {
                // Cheatsheet suppresses status popup (spec: mutually exclusive)
                if self.content == PopupContent::Cheatsheet && self.phase != PopupPhase::Hidden {
                    return;
                }
                if matches!(state.drawing, crate::state::DrawingState::Armed | crate::state::DrawingState::Drawing { .. }) {
                    let text = build_status_text(state.pinned_active, state.spotlight_active);
                    self.show_status(&text);
                }
            }
            InputEvent::ToggleHelp => {
                self.show_cheatsheet();
            }
            InputEvent::HideHelp => {
                self.hide_cheatsheet();
            }
            _ => {}
        }
    }

    pub fn show_status(&mut self, text: &str) {
        self.status_text = text.to_string();
        match &self.phase {
            PopupPhase::Hidden => {
                self.content = PopupContent::Status;
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now() };
            }
            PopupPhase::SlidingIn { .. } | PopupPhase::Holding { .. } => {
                // Update text in place, reset hold timer
                self.content = PopupContent::Status;
                self.phase = PopupPhase::Holding { started_at: Instant::now() };
            }
            PopupPhase::SlidingOut { from_y, .. } => {
                // Reverse: restart slide-in from current position
                self.content = PopupContent::Status;
                let current_y = self.current_y_offset();
                // For simplicity, restart SlidingIn — the animation starts from current visual position
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now() };
                // Store from_y for potential smooth transition (future optimization)
                let _ = (from_y, current_y);
            }
        }
    }

    pub fn show_cheatsheet(&mut self) {
        match &self.phase {
            PopupPhase::Hidden => {
                self.content = PopupContent::Cheatsheet;
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now() };
            }
            PopupPhase::SlidingIn { .. } | PopupPhase::Holding { .. } => {
                // Already showing — no-op for cheatsheet
                if self.content == PopupContent::Cheatsheet {
                    return;
                }
                // Status popup active — replace with cheatsheet
                self.content = PopupContent::Cheatsheet;
                self.phase = PopupPhase::Holding { started_at: Instant::now() };
            }
            PopupPhase::SlidingOut { .. } => {
                self.content = PopupContent::Cheatsheet;
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now() };
            }
        }
    }

    pub fn hide_cheatsheet(&mut self) {
        if self.content != PopupContent::Cheatsheet {
            return;
        }
        match &self.phase {
            PopupPhase::SlidingIn { .. } | PopupPhase::Holding { .. } => {
                let from_y = self.current_y_offset();
                self.phase = PopupPhase::SlidingOut { started_at: Instant::now(), from_y };
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let new_phase = match &self.phase {
            PopupPhase::SlidingIn { started_at } => {
                let elapsed = now.duration_since(*started_at).as_millis() as u64;
                if elapsed >= SLIDE_IN_DURATION_MS {
                    Some(PopupPhase::Holding { started_at: now })
                } else {
                    None
                }
            }
            PopupPhase::Holding { started_at } => {
                let elapsed = now.duration_since(*started_at).as_millis() as u64;
                if self.content == PopupContent::Cheatsheet {
                    None // cheatsheet has no hold timer
                } else if elapsed >= HOLD_DURATION_MS {
                    let from_y = self.current_y_offset();
                    Some(PopupPhase::SlidingOut { started_at: now, from_y })
                } else {
                    None
                }
            }
            PopupPhase::SlidingOut { started_at, .. } => {
                let elapsed = now.duration_since(*started_at).as_millis() as u64;
                if elapsed >= SLIDE_OUT_DURATION_MS {
                    Some(PopupPhase::Hidden)
                } else {
                    None
                }
            }
            PopupPhase::Hidden => None,
        };
        if let Some(phase) = new_phase {
            self.phase = phase;
        }
    }

    pub fn needs_frame(&self) -> bool {
        !matches!(self.phase, PopupPhase::Hidden)
    }

    pub fn is_visible(&self) -> bool {
        self.needs_frame()
    }

    pub fn content(&self) -> &PopupContent {
        &self.content
    }

    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    pub fn cheatsheet_rows(&self) -> &[(String, String)] {
        &self.cheatsheet_rows
    }

    pub fn current_y_offset(&self) -> f64 {
        match &self.phase {
            PopupPhase::Hidden => START_Y_OFFSET,
            PopupPhase::SlidingIn { started_at } => {
                let t = started_at.elapsed().as_secs_f64();
                animation::spring_position(t, START_Y_OFFSET, TARGET_Y_OFFSET, SLIDE_IN_OMEGA, SLIDE_IN_ZETA)
            }
            PopupPhase::Holding { .. } => TARGET_Y_OFFSET,
            PopupPhase::SlidingOut { started_at, from_y } => {
                let t = started_at.elapsed().as_secs_f64();
                animation::spring_position(t, *from_y, START_Y_OFFSET, SLIDE_OUT_OMEGA, SLIDE_OUT_ZETA)
            }
        }
    }
}
```

- [ ] **Step 2: Write PopupManager state machine tests**

Add to `src/popup/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DrawingState;

    fn make_manager() -> PopupManager {
        PopupManager::new("Alt")
    }

    fn armed_state() -> AppState {
        AppState { drawing: DrawingState::Armed, ..Default::default() }
    }

    // --- build_status_text ---

    #[test]
    fn build_status_text_transient() {
        assert_eq!(build_status_text(false, false), "Transient");
    }

    #[test]
    fn build_status_text_pinned() {
        assert_eq!(build_status_text(true, false), "Pinned");
    }

    #[test]
    fn build_status_text_spotlight() {
        assert_eq!(build_status_text(false, true), "Spotlight");
    }

    #[test]
    fn build_status_text_both() {
        assert_eq!(build_status_text(true, true), "Pinned \u{00b7} Spotlight");
    }

    // --- show_status from Hidden ---

    #[test]
    fn show_status_from_hidden_enters_sliding_in() {
        let mut m = make_manager();
        m.show_status("Pinned");
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
        assert_eq!(m.content, PopupContent::Status);
        assert_eq!(m.status_text, "Pinned");
    }

    // --- show_status from SlidingIn ---

    #[test]
    fn show_status_from_sliding_in_updates_text_resets_timer() {
        let mut m = make_manager();
        m.show_status("Pinned");
        std::thread::sleep(std::time::Duration::from_millis(50));
        m.show_status("Pinned \u{00b7} Spotlight");
        assert_eq!(m.status_text, "Pinned \u{00b7} Spotlight");
        assert!(matches!(m.phase, PopupPhase::Holding { .. }));
    }

    // --- show_status from SlidingOut ---

    #[test]
    fn show_status_from_sliding_out_reverses_to_sliding_in() {
        let mut m = make_manager();
        m.show_status("Pinned");
        // Fast-forward to Holding
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() - std::time::Duration::from_millis(2000) };
        m.tick(); // -> SlidingOut
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
        m.show_status("Spotlight");
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
    }

    // --- tick transitions ---

    #[test]
    fn tick_sliding_in_to_holding_after_duration() {
        let mut m = make_manager();
        m.show_status("Pinned");
        m.phase = PopupPhase::SlidingIn { started_at: std::time::Instant::now() - std::time::Duration::from_millis(500) };
        m.tick();
        assert!(matches!(m.phase, PopupPhase::Holding { .. }));
    }

    #[test]
    fn tick_holding_to_sliding_out_after_duration() {
        let mut m = make_manager();
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() - std::time::Duration::from_millis(1100) };
        m.tick();
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
    }

    #[test]
    fn tick_sliding_out_to_hidden_after_duration() {
        let mut m = make_manager();
        m.phase = PopupPhase::SlidingOut { started_at: std::time::Instant::now() - std::time::Duration::from_millis(400), from_y: 0.0 };
        m.tick();
        assert_eq!(m.phase, PopupPhase::Hidden);
    }

    // --- cheatsheet ---

    #[test]
    fn show_cheatsheet_from_hidden() {
        let mut m = make_manager();
        m.show_cheatsheet();
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
        assert_eq!(m.content, PopupContent::Cheatsheet);
    }

    #[test]
    fn show_cheatsheet_already_showing_is_noop() {
        let mut m = make_manager();
        m.show_cheatsheet();
        let phase_before = m.phase.clone();
        m.show_cheatsheet();
        assert_eq!(m.phase, phase_before);
    }

    #[test]
    fn hide_cheatsheet_triggers_sliding_out() {
        let mut m = make_manager();
        m.show_cheatsheet();
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() };
        m.hide_cheatsheet();
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
    }

    #[test]
    fn hide_cheatsheet_ignores_status_popup() {
        let mut m = make_manager();
        m.show_status("Pinned");
        let phase_before = m.phase.clone();
        m.hide_cheatsheet();
        assert_eq!(m.phase, phase_before);
    }

    #[test]
    fn cheatsheet_no_hold_timer() {
        let mut m = make_manager();
        m.show_cheatsheet();
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() - std::time::Duration::from_millis(5000) };
        m.tick();
        // Should still be Holding (no auto-dismiss for cheatsheet)
        assert!(matches!(m.phase, PopupPhase::Holding { .. }));
    }

    // --- on_event integration ---

    #[test]
    fn on_digit_pressed_shows_status() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::DigitPressed(1), &state);
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
    }

    #[test]
    fn on_toggle_help_shows_cheatsheet() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::ToggleHelp, &state);
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
        assert_eq!(m.content, PopupContent::Cheatsheet);
    }

    // --- needs_frame ---

    #[test]
    fn needs_frame_false_when_hidden() {
        let m = make_manager();
        assert!(!m.needs_frame());
    }

    #[test]
    fn needs_frame_true_when_sliding_in() {
        let mut m = make_manager();
        m.show_status("Pinned");
        assert!(m.needs_frame());
    }

    // --- cheatsheet_rows ---

    #[test]
    fn cheatsheet_rows_built_from_modifier() {
        let m = PopupManager::new("Ctrl");
        assert_eq!(m.cheatsheet_rows[0].0, "Ctrl + drag");
        assert_eq!(m.cheatsheet_rows[4].0, "Ctrl + `");
    }

    // --- cheatsheet suppresses status ---

    #[test]
    fn cheatsheet_suppresses_status_popup() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::ToggleHelp, &state);
        assert_eq!(m.content, PopupContent::Cheatsheet);
        // DigitPressed should NOT replace cheatsheet with status
        m.on_event(&InputEvent::DigitPressed(1), &state);
        assert_eq!(m.content, PopupContent::Cheatsheet);
    }

    // --- on_event with HideHelp ---

    #[test]
    fn on_hide_help_hides_cheatsheet() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::ToggleHelp, &state);
        assert!(m.needs_frame());
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() };
        m.on_event(&InputEvent::HideHelp, &state);
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
    }

    // --- on_event digit while idle is noop ---

    #[test]
    fn on_digit_pressed_idle_is_noop() {
        let mut m = make_manager();
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        m.on_event(&InputEvent::DigitPressed(1), &state);
        assert_eq!(m.phase, PopupPhase::Hidden);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib popup:: -- 2>&1 | tail -20`
Expected: FAIL (structs/methods don't exist yet)

- [ ] **Step 4: Run tests to verify they pass after implementation**

Run: `cargo test --lib popup:: -- 2>&1 | tail -20`
Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add src/popup/mod.rs
git commit -m "feat(popup): add PopupManager state machine with tests"
```

---

### Task 4: GDI Renderer

**Files:**
- Create: `src/popup/gdi_renderer.rs`

**Interfaces:**
- Produces: `GdiRenderer` with `new(hwnd: HWND) -> Self`, `render(manager: &PopupManager, monitor_rect: (i32,i32,i32,i32))`, `Drop`
- Consumes: `PopupManager::current_y_offset()`, `status_text()`, `content()`, `cheatsheet_rows()`

- [ ] **Step 1: Create gdi_renderer.rs with GDI resource management**

Create `src/popup/gdi_renderer.rs`:
```rust
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use super::{PopupManager, PopupContent};

const STATUS_HEIGHT: i32 = 44;
const STATUS_RADIUS: i32 = 12;
const STATUS_PADDING_H: i32 = 24;
const STATUS_TOP_MARGIN: i32 = 48;

const CHEATSHEET_WIDTH: i32 = 320;
const CHEATSHEET_ROW_HEIGHT: i32 = 32;
const CHEATSHEET_PADDING_V: i32 = 20;
const CHEATSHEET_RADIUS: i32 = 14;
const CHEATSHEET_PADDING_H: i32 = 24;

const BG_R: u8 = 28;
const BG_G: u8 = 28;
const BG_B: u8 = 30;
const BG_A_STATUS: u8 = 224;      // ~0.88
const BG_A_CHEATSHEET: u8 = 235;  // ~0.92

const SHADOW_COLOR: (u8, u8, u8) = (0, 0, 0);

pub struct GdiRenderer {
    hwnd: HWND,
    mem_dc: HDC,
    mem_bitmap: HBITMAP,
    original_stock_bitmap: HBITMAP,  // never deleted — restored in Drop
    font_normal: HFONT,
    font_key: HFONT,
    font_desc: HFONT,
    current_width: i32,
    current_height: i32,
    pixels: *mut u8,
}

impl GdiRenderer {
    pub fn new(hwnd: HWND) -> Self {
        unsafe {
            let screen_dc = GetDC(HWND::default()); // screen DC, not window DC
            let mem_dc = CreateCompatibleDC(Some(screen_dc));

            // Initial size — will be resized on first render
            let width = 400;
            let height = 300;

            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut pixels: *mut u8 = std::ptr::null_mut();
            let mem_bitmap = CreateDIBSection(Some(screen_dc), &bi, DIB_RGB_COLORS, &mut pixels as *mut *mut u8 as _, None, 0)
                .expect("CreateDIBSection failed");
            let original_stock_bitmap = SelectObject(mem_dc, mem_bitmap.into());

            let font_normal = create_font(14, FW_MEDIUM);
            let font_key = create_font(13, FW_SEMIBOLD);
            let font_desc = create_font(13, FW_NORMAL);

            ReleaseDC(HWND::default(), screen_dc);

            Self {
                hwnd,
                mem_dc,
                mem_bitmap,
                original_stock_bitmap,
                font_normal,
                font_key,
                font_desc,
                current_width: width,
                current_height: height,
                pixels,
            }
        }
    }

    fn ensure_size(&mut self, width: i32, height: i32) {
        if width == self.current_width && height == self.current_height {
            return;
        }
        unsafe {
            // Deselect current bitmap before deleting
            SelectObject(self.mem_dc, self.original_stock_bitmap);
            DeleteObject(self.mem_bitmap.into());

            let screen_dc = GetDC(HWND::default());
            let mut bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels: *mut u8 = std::ptr::null_mut();
            let mem_bitmap = CreateDIBSection(Some(screen_dc), &bi, DIB_RGB_COLORS, &mut pixels as *mut *mut u8 as _, None, 0)
                .expect("CreateDIBSection failed");
            // Select new bitmap — don't update original_stock_bitmap
            SelectObject(self.mem_dc, mem_bitmap.into());
            self.mem_bitmap = mem_bitmap;
            self.pixels = pixels;
            self.current_width = width;
            self.current_height = height;
            ReleaseDC(HWND::default(), screen_dc);
        }
    }

    pub fn render(&mut self, manager: &PopupManager, monitor_rect: (i32, i32, i32, i32)) {
        if !manager.is_visible() {
            unsafe { ShowWindow(self.hwnd, SW_HIDE); }
            return;
        }

        let (mon_left, mon_top, mon_right, mon_bottom) = monitor_rect;
        let mon_width = mon_right - mon_left;
        let y_offset = manager.current_y_offset() as i32;

        match manager.content() {
            PopupContent::Status => {
                self.render_status(manager, mon_left, mon_top, mon_width, y_offset);
            }
            PopupContent::Cheatsheet => {
                self.render_cheatsheet(manager, mon_left, mon_top, mon_width, mon_bottom, y_offset);
            }
        }
    }

    fn render_status(&mut self, manager: &PopupManager, mon_left: i32, mon_top: i32, mon_width: i32, y_offset: i32) {
        let text = manager.status_text();
        let text_w = measure_text_width(&self.mem_dc, self.font_normal, text);
        let popup_w = text_w + STATUS_PADDING_H * 2;
        let popup_h = STATUS_HEIGHT;

        // Add shadow margin
        let shadow_margin = 8;
        let buf_w = popup_w + shadow_margin * 2;
        let buf_h = popup_h + shadow_margin * 2;

        self.ensure_size(buf_w, buf_h);
        unsafe {
            clear_buffer(self.pixels, buf_w, buf_h);

            let card_x = shadow_margin;
            let card_y = shadow_margin;

            // Shadow layers
            paint_shadow(self.pixels, buf_w, buf_h, card_x + 2, card_y + 2, popup_w, popup_h, STATUS_RADIUS, SHADOW_COLOR, 60);

            // Card background
            paint_rounded_rect(self.pixels, buf_w, buf_h, card_x, card_y, popup_w, popup_h, STATUS_RADIUS, BG_R, BG_G, BG_B, BG_A_STATUS);

            // Text
            SelectObject(self.mem_dc, self.font_normal.into());
            SetBkMode(self.mem_dc, TRANSPARENT);
            SetTextColor(self.mem_dc, COLORREF(0x00FFFFFF)); // white
            let text_rect = RECT {
                left: card_x + STATUS_PADDING_H,
                top: card_y,
                right: card_x + popup_w - STATUS_PADDING_H,
                bottom: card_y + popup_h,
            };
            DrawTextW(self.mem_dc, &text.encode_utf16().collect::<Vec<_>>(), &mut text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // Position and show window
            let x = mon_left + (mon_width - buf_w) / 2;
            let y = mon_top + STATUS_TOP_MARGIN + y_offset;

            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(self.hwnd, Some(HWND_TOPMOST), x, y, buf_w, buf_h, SWP_NOACTIVATE);

            commit_layered(self.hwnd, self.mem_dc, buf_w, buf_h);
        }
    }

    fn render_cheatsheet(&mut self, manager: &PopupManager, mon_left: i32, mon_top: i32, mon_width: i32, mon_height: i32, y_offset: i32) {
        let rows = manager.cheatsheet_rows();
        let row_count = rows.len() as i32;
        let popup_w = CHEATSHEET_WIDTH;
        let popup_h = CHEATSHEET_PADDING_V * 2 + row_count * CHEATSHEET_ROW_HEIGHT;

        let shadow_margin = 12;
        let buf_w = popup_w + shadow_margin * 2;
        let buf_h = popup_h + shadow_margin * 2;

        self.ensure_size(buf_w, buf_h);
        unsafe {
            clear_buffer(self.pixels, buf_w, buf_h);

            let card_x = shadow_margin;
            let card_y = shadow_margin;

            // Shadow
            paint_shadow(self.pixels, buf_w, buf_h, card_x + 4, card_y + 4, popup_w, popup_h, CHEATSHEET_RADIUS, SHADOW_COLOR, 80);

            // Card background
            paint_rounded_rect(self.pixels, buf_w, buf_h, card_x, card_y, popup_w, popup_h, CHEATSHEET_RADIUS, BG_R, BG_G, BG_B, BG_A_CHEATSHEET);

            // Text rows
            for (i, (key, desc)) in rows.iter().enumerate() {
                let row_y = card_y + CHEATSHEET_PADDING_V + i as i32 * CHEATSHEET_ROW_HEIGHT;

                // Key (left-aligned, semibold)
                SelectObject(self.mem_dc, self.font_key.into());
                SetBkMode(self.mem_dc, TRANSPARENT);
                SetTextColor(self.mem_dc, COLORREF(0x00E5E5E5)); // #E5E5E5
                let key_rect = RECT {
                    left: card_x + CHEATSHEET_PADDING_H,
                    top: row_y,
                    right: card_x + popup_w / 2,
                    bottom: row_y + CHEATSHEET_ROW_HEIGHT,
                };
                let key_w: Vec<u16> = key.encode_utf16().collect();
                DrawTextW(self.mem_dc, &key_w, &mut key_rect.into(), DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                // Desc (right-aligned, regular)
                SelectObject(self.mem_dc, self.font_desc.into());
                SetTextColor(self.mem_dc, COLORREF(0x00AEAEB2)); // #AEAEB2
                let desc_rect = RECT {
                    left: card_x + popup_w / 2,
                    top: row_y,
                    right: card_x + popup_w - CHEATSHEET_PADDING_H,
                    bottom: row_y + CHEATSHEET_ROW_HEIGHT,
                };
                let desc_w: Vec<u16> = desc.encode_utf16().collect();
                DrawTextW(self.mem_dc, &desc_w, &mut desc_rect.into(), DT_RIGHT | DT_VCENTER | DT_SINGLELINE);
            }

            // Position: centered on monitor
            let x = mon_left + (mon_width - buf_w) / 2;
            let y = mon_top + (mon_height - buf_h) / 2 + y_offset;

            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            SetWindowPos(self.hwnd, Some(HWND_TOPMOST), x, y, buf_w, buf_h, SWP_NOACTIVATE);

            commit_layered(self.hwnd, self.mem_dc, buf_w, buf_h);
        }
    }
}

impl Drop for GdiRenderer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem_dc, self.original_stock_bitmap);
            DeleteObject(self.mem_bitmap.into());
            DeleteDC(self.mem_dc);
            DeleteObject(self.font_normal.into());
            DeleteObject(self.font_key.into());
            DeleteObject(self.font_desc.into());
        }
    }
}

unsafe fn create_font(size: i32, weight: i32) -> HFONT {
    let face_name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
    CreateFontW(
        -size, 0, 0, 0, weight, 0, 0, 0,
        DEFAULT_CHARSET.0.into(), OUT_DEFAULT_PRECIS.0.into(),
        CLIP_DEFAULT_PRECIS.0.into(), CLEARTYPE_QUALITY.0.into(),
        DEFAULT_PITCH.0.into(),
        windows::core::PCWSTR(face_name.as_ptr()),
    ).expect("CreateFontW failed")
}

unsafe fn clear_buffer(pixels: *mut u8, width: i32, height: i32) {
    let total = (width * height * 4) as usize;
    std::ptr::write_bytes(pixels, 0, total);
}

unsafe fn paint_rounded_rect(pixels: *mut u8, buf_w: i32, buf_h: i32, x: i32, y: i32, w: i32, h: i32, radius: i32, r: u8, g: u8, b: u8, a: u8) {
    for py in 0..h {
        for px in 0..w {
            let corner_alpha = rounded_corner_alpha(px, py, w, h, radius);
            if corner_alpha <= 0 {
                continue;
            }
            let alpha = (a as f32 * corner_alpha) as u8;
            let idx = (((y + py) * buf_w + (x + px)) * 4) as usize;
            if idx + 3 >= (buf_w * buf_h * 4) as usize {
                continue;
            }
            blend_pixel(pixels.add(idx), r, g, b, alpha);
        }
    }
}

unsafe fn paint_shadow(pixels: *mut u8, buf_w: i32, buf_h: i32, x: i32, y: i32, w: i32, h: i32, radius: i32, color: (u8, u8, u8), base_alpha: u8) {
    // Layered shadow: 3 layers at increasing offsets and decreasing alpha
    for i in 0..3 {
        let offset = (i + 1) as i32;
        let alpha = (base_alpha as u32 * (3 - i) as u32 / 3) as u8;
        paint_rounded_rect(pixels, buf_w, buf_h, x + offset, y + offset, w, h, radius + offset, color.0, color.1, color.2, alpha);
    }
}

fn rounded_corner_alpha(px: i32, py: i32, w: i32, h: i32, radius: i32) -> f32 {
    let r = radius as f32;
    let (cx, cy) = if px < radius && py < radius {
        (r, r) // top-left
    } else if px >= w - radius && py < radius {
        (w as f32 - r - 1.0, r) // top-right
    } else if px < radius && py >= h - radius {
        (r, h as f32 - r - 1.0) // bottom-left
    } else if px >= w - radius && py >= h - radius {
        (w as f32 - r - 1.0, h as f32 - r - 1.0) // bottom-right
    } else {
        return 1.0; // not in a corner
    };
    let dx = px as f32 - cx;
    let dy = py as f32 - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= r - 1.0 {
        1.0
    } else if dist >= r + 1.0 {
        0.0
    } else {
        (r + 1.0 - dist) / 2.0 // smooth 2px anti-alias
    }
}

unsafe fn blend_pixel(dst: *mut u8, r: u8, g: u8, b: u8, a: u8) {
    if a == 0 { return; }
    let alpha = a as f32 / 255.0;
    let inv = 1.0 - alpha;
    *dst = (b as f32 * alpha + *dst as f32 * inv) as u8;       // B
    *dst.add(1) = (g as f32 * alpha + *dst.add(1) as f32 * inv) as u8; // G
    *dst.add(2) = (r as f32 * alpha + *dst.add(2) as f32 * inv) as u8; // R
    *dst.add(3) = (a as f32 + *dst.add(3) as f32 * inv).min(255.0) as u8; // A
}

/// Push pixels to the layered window. mem_dc already has mem_bitmap selected.
unsafe fn commit_layered(hwnd: HWND, mem_dc: HDC, width: i32, height: i32) {
    let size = SIZE { cx: width, cy: height };
    let point = POINT { x: 0, y: 0 };
    let mut blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER.0,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA.0,
    };
    let _ = UpdateLayeredWindow(
        hwnd,
        None,
        None,
        Some(&size),
        Some(mem_dc),
        Some(&point),
        COLORREF(0),
        Some(&mut blend),
        ULW_ALPHA,
    );
}

fn measure_text_width(dc: HDC, font: HFONT, text: &str) -> i32 {
    unsafe {
        let old = SelectObject(dc, font.into());
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
        SelectObject(dc, old);
        size.cx
    }
}
```

- [ ] **Step 2: Register module in popup/mod.rs**

Add to the top of `src/popup/mod.rs`:
```rust
#[cfg(windows)]
pub mod gdi_renderer;
```

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build 2>&1 | head -20`
Expected: compiles (may have warnings about unused)

- [ ] **Step 4: Commit**

```bash
git add src/popup/gdi_renderer.rs src/popup/mod.rs
git commit -m "feat(popup): add GDI renderer with rounded rects and text"
```

---

### Task 5: Integration — Wire Popup into App

**Files:**
- Modify: `src/overlay.rs` (App struct, new, resumed, about_to_wait)

**Interfaces:**
- Consumes: `PopupManager::new()`, `GdiRenderer::new()`, `PopupManager::on_event()`, `PopupManager::tick()`, `PopupManager::needs_frame()`
- Consumes: Win32 `GetCursorPos`, `MonitorFromPoint`, `GetMonitorInfoW`

- [ ] **Step 1: Add modifier_name to AppConfig**

In `src/config.rs`, add `modifier_name` field to `AppConfig`:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub modifier_vk_codes: Vec<u32>,
    pub border_width: i32,
    pub color_mode: ColorMode,
    pub modifier_name: String,
}
```

Update `Default` impl:
```rust
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            modifier_vk_codes: modifier_vk_codes("Alt"),
            border_width: 4,
            color_mode: ColorMode::Solid { r: 255, g: 0, b: 0 },
            modifier_name: "Alt".to_string(),
        }
    }
}
```

Update `parse()` to store the name:
```rust
let modifier_str = raw.modifier.as_deref().unwrap_or("Alt");
let modifier_vk_codes = modifier_vk_codes(modifier_str);
let modifier_name = modifier_str.to_string();
// ...
Self {
    modifier_vk_codes,
    border_width,
    color_mode,
    modifier_name,
}
```

Run: `cargo test --lib config -- 2>&1 | tail -10`
Expected: all existing tests pass (update any that construct `AppConfig` directly).

Commit:
```bash
git add src/config.rs
git commit -m "feat(config): add modifier_name field to AppConfig"
```

- [ ] **Step 2: Add popup fields to App struct**

In `src/overlay.rs`, modify the `App` struct (line 118):
```rust
pub struct App {
    window: Option<Window>,
    state: AppState,
    input_rx: Receiver<InputEvent>,
    border_width: i32,
    color_mode: ColorMode,
    #[cfg(windows)]
    dib_cache: Option<DibCache>,
    // Popup system
    #[cfg(windows)]
    popup_hwnd: Option<HWND>,
    popup_manager: PopupManager,
    #[cfg(windows)]
    popup_renderer: Option<GdiRenderer>,
    popup_monitor_rect: (i32, i32, i32, i32), // cached at show time
}
```

Add imports at top of `overlay.rs`:
```rust
use crate::popup::PopupManager;
#[cfg(windows)]
use crate::popup::gdi_renderer::GdiRenderer;
```

- [ ] **Step 3: Update App::new to initialize popup fields**

Modify `App::new`:
```rust
pub fn new(input_rx: Receiver<InputEvent>, border_width: i32, color_mode: ColorMode, modifier_name: String) -> Self {
    Self {
        window: None,
        state: AppState::default(),
        input_rx,
        border_width,
        color_mode,
        #[cfg(windows)]
        dib_cache: None,
        #[cfg(windows)]
        popup_hwnd: None,
        popup_manager: PopupManager::new(&modifier_name),
        #[cfg(windows)]
        popup_renderer: None,
        popup_monitor_rect: (0, 0, 1920, 1080),
    }
}
```

Update the caller in `main.rs` that creates `App::new()` to pass the modifier name from config.

- [ ] **Step 4: Create popup window in App::resumed**

Add after the overlay window creation in `resumed()`:
```rust
#[cfg(windows)]
{
    use windows::Win32::UI::WindowsAndMessaging::*;
    let class_name: Vec<u16> = "HoldRectPopup\0".encode_utf16().collect();
    let window_name: Vec<u16> = "HoldRectPopup\0".encode_utf16().collect();

    // Register window class
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(DefWindowProcW),
        hInstance: HINSTANCE::default(),
        lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    RegisterClassExW(&wc);

    let popup_hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        windows::core::PCWSTR(class_name.as_ptr()),
        windows::core::PCWSTR(window_name.as_ptr()),
        WS_POPUP,
        0, 0, 400, 300, // will be resized on render
        None, None, HINSTANCE::default(), None,
    ).expect("Failed to create popup window");

    self.popup_hwnd = Some(popup_hwnd);
    self.popup_renderer = Some(GdiRenderer::new(popup_hwnd));
}
```

- [ ] **Step 5: Wire popup into about_to_wait**

Modify `about_to_wait` to:
1. Route events to popup_manager
2. Tick animation
3. Render popup
4. Include popup in animation control flow

```rust
fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    // Drain all pending input events
    while let Ok(event) = self.input_rx.try_recv() {
        let new_state = process_event(&self.state, &event);
        let was_hidden = !self.popup_manager.needs_frame();
        self.state = new_state;
        // Route to popup manager
        self.popup_manager.on_event(&event, &self.state);
        // Cache monitor rect when popup transitions from Hidden -> visible
        if was_hidden && self.popup_manager.needs_frame() {
            #[cfg(windows)]
            { self.popup_monitor_rect = get_cursor_monitor_work_area(); }
        }
    }

    self.render();

    // Popup animation tick + render
    if self.popup_manager.needs_frame() {
        self.popup_manager.tick();
        #[cfg(windows)]
        if let (Some(renderer), Some(_hwnd)) = (self.popup_renderer.as_mut(), self.popup_hwnd) {
            renderer.render(&self.popup_manager, self.popup_monitor_rect);
        }
    }

    let needs_animation = matches!(&self.state.drawing, DrawingState::Drawing { .. })
        || !self.state.pinned_rects.is_empty()
        || self.popup_manager.needs_frame(); // keep event loop alive for popup animation

    if needs_animation {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    } else {
        event_loop.set_control_flow(ControlFlow::Wait);
    }
}
```

Add the multi-monitor helper function in `overlay.rs`:
```rust
#[cfg(windows)]
fn get_cursor_monitor_work_area() -> (i32, i32, i32, i32) {
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(hmon, &mut info);
        let work = info.rcWork;
        (work.left, work.top, work.right, work.bottom)
    }
}
```

- [ ] **Step 6: Update main.rs caller**

Find where `App::new(...)` is called in `main.rs` and pass the modifier name from the config:
```rust
let mut app = App::new(input_rx, border_width, color_mode, config.modifier_name.clone());
```

- [ ] **Step 7: Build and test**

Run: `cargo build 2>&1 | head -30`
Expected: compiles successfully

Run: `cargo test --lib 2>&1 | tail -20`
Expected: all tests pass

- [ ] **Step 8: Commit**

```bash
git add src/overlay.rs src/main.rs src/popup/mod.rs src/config.rs
git commit -m "feat(popup): wire popup system into App event loop"
```

---

### Task 6: Manual Verification

- [ ] **Step 1: Build release binary**

Run: `cargo build --release 2>&1 | tail -5`

- [ ] **Step 2: Test status popup**

1. Run `target/release/holdrect.exe`
2. Hold Alt + left-click drag → drawing should work as before
3. While holding Alt, press `1` → status popup "Pinned" appears at top center with spring animation
4. Press `2` → popup updates to "Pinned · Spotlight"
5. Press `1` again → popup updates to "Spotlight"
6. Wait 1s → popup slides out

- [ ] **Step 3: Test cheatsheet popup**

1. Hold Alt + press backtick (`` ` ``) → cheatsheet popup appears centered
2. Release backtick → cheatsheet slides out
3. Hold Alt + backtick again → appears again
4. Release Alt while holding backtick → cheatsheet hides

- [ ] **Step 4: Test multi-monitor** (if applicable)

1. Move cursor to second monitor
2. Hold Alt + press `1` → popup should appear on second monitor

- [ ] **Step 5: Test mouse passthrough**

1. Hold Alt + drag to draw a rect
2. While drawing, press `1` → popup appears but drag continues uninterrupted
3. Popup should not intercept any mouse events

- [ ] **Step 6: Commit any fixes**

```bash
git add -A
git commit -m "fix(popup): manual verification fixes"
```
