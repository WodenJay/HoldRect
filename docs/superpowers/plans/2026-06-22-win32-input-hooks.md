# Win32 Input Hooks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace rdev passive listener with Win32 SetWindowsHookExW to intercept and suppress Ctrl+mouse input, fixing system selection box appearing alongside HoldRect and enabling overlay to work in all apps.

**Architecture:** New `src/hook.rs` module installs WH_KEYBOARD_LL + WH_MOUSE_LL hooks on a background thread. Pure decision functions (`decide_keyboard`, `decide_mouse`) handle all logic and are unit-testable. Two AtomicBools (`SHOULD_SUPPRESS`, `DRAG_IN_PROGRESS`) coordinate suppression state. Old `src/input.rs` and rdev dependency removed.

**Tech Stack:** Rust, `windows` crate (already in Cargo.toml), `mpsc::Sender`, `winit::EventLoopProxy`

**Spec:** `docs/superpowers/specs/2026-06-22-win32-input-hooks-design.md`

## Global Constraints

- `cargo build` / `cargo test` max concurrency = 1 (low memory)
- No `rust-analyzer`, `cargo watch`, `clippy --watch`
- Prefer `cargo test --lib` for fast iteration; full `cargo test` only for final check
- `#[cfg(windows)]` gate on hook.rs and related code
- Commit after each task

---

### Task 1: Pure Decision Functions with Tests (TDD)

**Files:**
- Modify: `src/input.rs` → rename to `src/hook.rs` (or create new file; input.rs deleted in Task 3)

**Interfaces:**
- Produces:
  - `pub fn decide_keyboard(vk_code: u32, is_key_down: bool) -> Option<InputEvent>`
  - `pub fn decide_mouse(msg: u32, pt: (i32, i32), should_suppress: bool, drag_in_progress: bool, ctrl_held: bool) -> (Option<InputEvent>, bool)`

- [ ] **Step 1: Write failing tests for `decide_keyboard`**

Create `src/hook.rs` with:

```rust
use crate::state::InputEvent;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// Pure decision function — no Win32 side effects, fully unit-testable
pub fn decide_keyboard(vk_code: u32, is_key_down: bool) -> Option<InputEvent> {
    let is_ctrl = vk_code == VK_LCONTROL.0 as u32 || vk_code == VK_RCONTROL.0 as u32;
    if !is_ctrl {
        return None;
    }
    Some(InputEvent::ModifierChanged { pressed: is_key_down })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_down_returns_modifier_pressed() {
        let result = decide_keyboard(VK_LCONTROL.0 as u32, true);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn ctrl_up_returns_modifier_released() {
        let result = decide_keyboard(VK_LCONTROL.0 as u32, false);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: false }));
    }

    #[test]
    fn right_ctrl_down_returns_modifier_pressed() {
        let result = decide_keyboard(VK_RCONTROL.0 as u32, true);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn non_ctrl_key_returns_none() {
        let result = decide_keyboard(VK_LSHIFT.0 as u32, true);
        assert_eq!(result, None);
    }

    #[test]
    fn non_ctrl_key_up_returns_none() {
        let result = decide_keyboard(0x41, false); // 'A' key
        assert_eq!(result, None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib hook::tests 2>&1`
Expected: FAIL — `hook` module not in `mod` tree yet.

- [ ] **Step 3: Add `mod hook;` to `src/main.rs`**

Add `mod hook;` after existing module declarations (line 6):

```rust
mod state;
mod input;
mod overlay;
mod tray;
mod hook;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib hook::tests 2>&1`
Expected: PASS (5 tests)

- [ ] **Step 5: Commit**

```bash
git add src/hook.rs src/main.rs
git commit -m "feat: add decide_keyboard pure function with tests"
```

---

### Task 2: Add `decide_mouse` Pure Function with Tests (TDD)

**Files:**
- Modify: `src/hook.rs`

**Interfaces:**
- Produces: `pub fn decide_mouse(msg: u32, pt: (i32, i32), should_suppress: bool, drag_in_progress: bool, ctrl_held: bool) -> (Option<InputEvent>, bool)`

- [ ] **Step 1: Write failing tests for `decide_mouse`**

Add to `src/hook.rs` (below `decide_keyboard` and its tests):

```rust
/// Pure decision function for mouse events.
/// Returns (Option<InputEvent>, should_suppress_this_event).
pub fn decide_mouse(
    msg: u32,
    pt: (i32, i32),
    should_suppress: bool,
    drag_in_progress: bool,
    ctrl_held: bool,
) -> (Option<InputEvent>, bool) {
    // If drag in progress, only LeftUp matters (to end the drag)
    if drag_in_progress {
        if msg == WM_LBUTTONUP {
            return (Some(InputEvent::MouseButtonUp { x: pt.0, y: pt.1 }), true);
        }
        if msg == WM_MOUSEMOVE {
            return (Some(InputEvent::MouseMove { x: pt.0, y: pt.1 }), true);
        }
        // Other events during drag: pass through (don't suppress right-click etc.)
        return (None, false);
    }

    // No drag in progress: only act if suppress mode active and Ctrl held
    if !should_suppress || !ctrl_held {
        return (None, false);
    }

    // should_suppress && ctrl_held
    if msg == WM_LBUTTONDOWN {
        return (Some(InputEvent::MouseButtonDown { x: pt.0, y: pt.1 }), true);
    }

    (None, false)
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib hook::tests::decide_mouse 2>&1`
Expected: compile may succeed (tests don't exist yet) — proceed to add tests.

- [ ] **Step 3: Add `decide_mouse` tests**

Add inside `mod tests` in `src/hook.rs`:

```rust
    // --- decide_mouse tests ---

    #[test]
    fn mouse_leftdown_no_suppress_passes_through() {
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (100, 200), false, false, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_leftdown_suppress_ctrl_held_is_suppressed() {
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (100, 200), true, false, true);
        assert_eq!(event, Some(InputEvent::MouseButtonDown { x: 100, y: 200 }));
        assert!(suppress);
    }

    #[test]
    fn mouse_leftdown_suppress_but_no_ctrl_passes_through() {
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (100, 200), true, false, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_move_drag_in_progress_is_suppressed() {
        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (300, 400), false, true, false);
        assert_eq!(event, Some(InputEvent::MouseMove { x: 300, y: 400 }));
        assert!(suppress);
    }

    #[test]
    fn mouse_move_no_drag_passes_through() {
        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (300, 400), true, false, true);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_leftup_drag_in_progress_is_suppressed() {
        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (500, 600), false, true, false);
        assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 500, y: 600 }));
        assert!(suppress);
    }

    #[test]
    fn mouse_leftup_no_drag_passes_through() {
        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (500, 600), true, false, true);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_rightdown_suppress_passes_through() {
        let (event, suppress) = decide_mouse(WM_RBUTTONDOWN, (100, 200), true, false, true);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_rightdown_drag_in_progress_passes_through() {
        let (event, suppress) = decide_mouse(WM_RBUTTONDOWN, (100, 200), false, true, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib hook::tests 2>&1`
Expected: PASS (14 tests)

- [ ] **Step 5: Commit**

```bash
git add src/hook.rs
git commit -m "feat: add decide_mouse pure function with tests"
```

---

### Task 3: Implement `start_hook_listener` (Integration Layer)

**Files:**
- Modify: `src/hook.rs`

**Interfaces:**
- Consumes: `decide_keyboard`, `decide_mouse` from same module
- Produces: `pub fn start_hook_listener(tx: Sender<InputEvent>, proxy: EventLoopProxy<()>)`

- [ ] **Step 1: Add statics and `start_hook_listener` to `src/hook.rs`**

Add at the top of `src/hook.rs` (after imports):

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;

use windows::Win32::Foundation::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use winit::event_loop::EventLoopProxy;

use crate::state::InputEvent;

static SHOULD_SUPPRESS: AtomicBool = AtomicBool::new(false);
static DRAG_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static TX: OnceLock<Sender<InputEvent>> = OnceLock::new();
static PROXY: OnceLock<EventLoopProxy<()>> = OnceLock::new();

/// Start global input hook listener on a background thread.
/// Replaces rdev — intercepts and suppresses Ctrl+mouse combo.
pub fn start_hook_listener(tx: Sender<InputEvent>, proxy: EventLoopProxy<()>) {
    TX.set(tx).expect("start_hook_listener called twice");
    PROXY.set(proxy).expect("start_hook_listener called twice");

    std::thread::spawn(move || unsafe {
        let _keyboard = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            HINSTANCE::default(),
            0,
        )
        .expect("Failed to install keyboard hook");

        let _mouse = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_hook_proc),
            HINSTANCE::default(),
            0,
        )
        .expect("Failed to install mouse hook");

        // Message loop required for low-level hooks to receive callbacks
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
            // hooks are active as long as message loop runs
        }
    });
}
```

- [ ] **Step 2: Add keyboard hook proc**

```rust
unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let is_key_down = w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize;

        if let Some(event) = decide_keyboard(kb.vkCode, is_key_down) {
            SHOULD_SUPPRESS.store(is_key_down, Ordering::Relaxed);
            if let Some(tx) = TX.get() {
                let _ = tx.send(event);
            }
            if let Some(proxy) = PROXY.get() {
                let _ = proxy.send_event(());
            }
        }
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}
```

- [ ] **Step 3: Add mouse hook proc**

```rust
unsafe extern "system" fn mouse_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let ms = *(l_param.0 as *const MSLLHOOKSTRUCT);
        let msg = w_param.0 as u32;
        let pt = (ms.pt.x, ms.pt.y);
        let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) & 0x8000u16 as i16 != 0;
        let suppress = SHOULD_SUPPRESS.load(Ordering::Relaxed);
        let drag = DRAG_IN_PROGRESS.load(Ordering::Relaxed);

        let (event, should_suppress) = decide_mouse(msg, pt, suppress, drag, ctrl);

        // Update DRAG_IN_PROGRESS based on decision
        if should_suppress {
            match msg {
                WM_LBUTTONDOWN => { DRAG_IN_PROGRESS.store(true, Ordering::Relaxed); }
                WM_LBUTTONUP   => { DRAG_IN_PROGRESS.store(false, Ordering::Relaxed); }
                _ => {}
            }
        }

        if let Some(event) = event {
            if let Some(tx) = TX.get() {
                let _ = tx.send(event);
            }
            if let Some(proxy) = PROXY.get() {
                let _ = proxy.send_event(());
            }
        }

        if should_suppress {
            return LRESULT(1); // suppress event
        }
    }
    CallNextHookEx(None, n_code, w_param, l_param)
}
```

- [ ] **Step 4: Run full tests**

Run: `cargo test --lib 2>&1`
Expected: PASS (14 hook tests + existing state/tray tests)

- [ ] **Step 5: Commit**

```bash
git add src/hook.rs
git commit -m "feat: implement start_hook_listener with Win32 hooks"
```

---

### Task 4: Wire Hook into Main and Remove rdev

**Files:**
- Modify: `src/main.rs`
- Modify: `Cargo.toml`
- Delete: `src/input.rs`

**Interfaces:**
- Consumes: `hook::start_hook_listener(tx, proxy)` from Task 3

- [ ] **Step 1: Update `src/main.rs`**

Replace the entire file with:

```rust
#![windows_subsystem = "windows"]

mod state;
mod overlay;
mod tray;
#[cfg(windows)]
mod hook;

use std::sync::mpsc;
use std::thread;

use crate::overlay::{create_event_loop, run_overlay};
use crate::state::InputEvent;
use crate::tray::{start_tray, AppExit};

fn main() {
    #[cfg(windows)]
    set_dpi_awareness();

    let (event_loop, proxy) = create_event_loop();
    let (input_tx, input_rx) = mpsc::channel::<InputEvent>();
    let (exit_tx, exit_rx) = mpsc::channel::<AppExit>();

    // Start Win32 input hook listener (replaces rdev)
    #[cfg(windows)]
    crate::hook::start_hook_listener(input_tx, proxy);

    let _tray_icon = start_tray(exit_tx);

    thread::spawn(move || {
        let _ = exit_rx.recv();
        std::process::exit(0);
    });

    run_overlay(event_loop, input_rx);
    std::process::exit(0);
}

#[cfg(windows)]
fn set_dpi_awareness() {
    use windows::Win32::UI::HiDpi::*;
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
    }
}
```

- [ ] **Step 2: Delete `src/input.rs`**

Run: `rm src/input.rs`

- [ ] **Step 3: Remove rdev from Cargo.toml**

Remove this line from `[dependencies]`:
```
rdev = "0.5"
```

- [ ] **Step 4: Build and run tests**

Run: `cargo test --lib 2>&1`
Expected: PASS

Run: `cargo build 2>&1`
Expected: PASS (no warnings about unused imports or dead code)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: wire Win32 hooks, remove rdev dependency"
```

---

### Task 5: Manual Integration Test

**Files:** None (verification only)

- [ ] **Step 1: Build release**

Run: `cargo build --release 2>&1`
Expected: PASS

- [ ] **Step 2: Run and test on Windows desktop**

Run: `cargo run --release`
Then:
1. Press and hold Ctrl
2. Left-click and drag on desktop → ONLY HoldRect red box appears (no system selection box)
3. Release mouse → red box disappears
4. Release Ctrl

- [ ] **Step 3: Test in File Explorer**

1. Open any folder in Explorer
2. Ctrl+LeftDrag → only HoldRect red box, no Explorer text/file selection

- [ ] **Step 4: Test in browser**

1. Open any web page
2. Ctrl+LeftDrag → only HoldRect red box, no browser text selection

- [ ] **Step 5: Test Ctrl shortcuts still work**

1. In any text editor, Ctrl+C / Ctrl+V / Ctrl+Z all work normally
2. Normal mouse usage (no Ctrl) completely unaffected

- [ ] **Step 6: Test Ctrl release before mouse release**

1. Ctrl+LeftDrag to start drawing
2. Release Ctrl while still holding mouse button
3. Release mouse → no orphaned events reach foreground app

- [ ] **Step 7: Commit if any fixes needed**

```bash
git add -A
git commit -m "fix: integration test adjustments"
```
