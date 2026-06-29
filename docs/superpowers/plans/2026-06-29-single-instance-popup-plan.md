# Single Instance + Popup Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge PR #3, then refactor single-instance notifications from MessageBox to slide-in popups, fix error handling, and undo cargo fmt noise.

**Architecture:** Merge PR as-is first. Then undo cargo fmt changes by re-running `cargo fmt` on main (since fmt is idempotent, the "noise" becomes baseline). Refactor `single_instance.rs` to remove MessageBox, add `Result`-based `try_acquire`, add `notify_existing_instance()` using `FindWindow` + `PostMessage`. The popup window's `WndProc` handles the custom message via `RegisterWindowMessageW` and forwards via `hook::send_event()`. Two new `InputEvent` variants (`FirstLaunch`, `InstanceAlreadyRunning`) trigger `popup_manager.show_status()` in the overlay event loop.

**Tech Stack:** Rust, Windows API (CreateMutexW, FindWindowW, PostMessageW, RegisterWindowMessageW), existing PopupManager/GdiRenderer.

## Global Constraints

- `cargo build` / `cargo test` max concurrency = 1 (`-j1`)
- No `rust-analyzer`, `cargo check --watch`, `clippy --all-targets` background tasks
- Run minimal `cargo test` scope during dev; full `cargo test` only for final confirmation
- TDD: write failing test first, then implement
- Commit after each task; commit messages don't start with "@"
- Git author: WodenJay <wodenjay@gmail.com>, no Co-Author lines

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/single_instance.rs` | Mutex acquire with `Result`, `notify_existing_instance()` with PostMessage |
| `src/state.rs` | `InputEvent` enum with `FirstLaunch` + `InstanceAlreadyRunning` variants |
| `src/hook.rs` | `pub(crate) fn send_event()` to forward events from WndProc |
| `src/overlay.rs` | Popup WndProc handles custom registered message, overlay event handler shows popups |
| `src/main.rs` | Wire single_instance check, send `FirstLaunch`, call `notify_existing_instance` on AlreadyRunning |
| `src/magnifier.rs` | Flicker fix + toggle mode (from PR, kept) |
| `README.md` | Fix `Quick Start/` typo |

---

### Task 1: Merge PR #3

**Files:**
- Modify: all 15 files in PR diff

**Interfaces:**
- Consumes: PR branch `pr-3` (already fetched)
- Produces: main branch with all PR changes merged

- [ ] **Step 1: Merge PR #3 into main**

```bash
git checkout main
git merge pr-3 --no-ff -m "Merge PR #3: single instance, magnifier flicker fix, toggle mode, digit key suppression"
```

- [ ] **Step 2: Verify build**

```bash
cargo build -j1
```

Expected: BUILD SUCCEEDED

- [ ] **Step 3: Run tests**

```bash
cargo test -j1
```

Expected: all tests pass

- [ ] **Step 4: Verify merge commit exists**

```bash
git log --oneline -3
```

---

### Task 2: Fix README typo + establish fmt baseline

**Files:**
- Modify: `README.md`

**Interfaces:**
- Produces: clean README, verified fmt baseline

- [ ] **Step 1: Fix README typo**

Change `## Quick Start/` → `## Quick Start` (trailing slash removal)

- [ ] **Step 2: Verify cargo fmt produces no diff**

```bash
cargo fmt
git diff
```

Expected: empty diff (fmt was already applied by PR, idempotent)

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "fix: remove trailing slash from Quick Start heading"
```

---

### Task 3: Add `FirstLaunch` and `InstanceAlreadyRunning` to InputEvent + tests

**Files:**
- Modify: `src/state.rs`

**Interfaces:**
- Consumes: existing `InputEvent` enum, `process_event` function
- Produces: `InputEvent::FirstLaunch`, `InputEvent::InstanceAlreadyRunning` — both no-ops in `process_event`

- [ ] **Step 1: Write failing tests for new InputEvent variants**

In `src/state.rs` `mod tests` section, add:

```rust
#[test]
fn first_launch_is_noop() {
    let state = AppState {
        drawing: DrawingState::Idle,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::FirstLaunch);
    assert_eq!(next.drawing, DrawingState::Idle);
    assert_eq!(next, state);
}

#[test]
fn instance_already_running_is_noop() {
    let state = AppState {
        drawing: DrawingState::Armed,
        pinned_active: true,
        ..Default::default()
    };
    let next = process_event(&state, &InputEvent::InstanceAlreadyRunning);
    assert_eq!(next, state);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -j1 -- first_launch_is_noop
```

Expected: COMPILE ERROR — `FirstLaunch` doesn't exist yet

- [ ] **Step 3: Add variants to InputEvent enum**

In `src/state.rs`, add to `InputEvent`:

```rust
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    DigitPressed(u8),
    EscapePressed,
    ToggleHelp,             // modifier + ` pressed
    HideHelp,               // modifier or ` released
    ScrollUp,               // magnifier zoom in
    ScrollDown,             // magnifier zoom out
    FirstLaunch,            // first instance started
    InstanceAlreadyRunning, // another instance tried to start
}
```

The existing `_ =>` catch-all arm in `process_event` handles these as no-ops automatically.

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -j1 -- first_launch_is_noop instance_already_running_is_noop
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/state.rs
git commit -m "feat: add FirstLaunch and InstanceAlreadyRunning to InputEvent"
```

---

### Task 4: Add `hook::send_event()` public helper

**Files:**
- Modify: `src/hook.rs`

**Interfaces:**
- Consumes: private `TX` and `PROXY` statics in hook.rs
- Produces: `pub(crate) fn send_event(event: InputEvent)` — sends event + wakes event loop

- [ ] **Step 1: Add `send_event` function to hook.rs**

After `update_modifier_codes`, add:

```rust
/// Send an InputEvent from outside the hook thread (e.g., from a WndProc).
/// Used for forwarding custom Windows messages to the main event loop.
pub(crate) fn send_event(event: InputEvent) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(event);
    }
    if let Some(proxy) = PROXY.get() {
        let _ = proxy.send_event(());
    }
}
```

- [ ] **Step 2: Verify build**

```bash
cargo build -j1
```

Expected: BUILD SUCCEEDED

- [ ] **Step 3: Commit**

```bash
git add src/hook.rs
git commit -m "feat: add hook::send_event for forwarding events from WndProc"
```

---

### Task 5: Rewrite `single_instance.rs` — remove MessageBox, add Result + PostMessage

**Files:**
- Modify: `src/single_instance.rs`

**Interfaces:**
- Consumes: `windows::Win32::System::Threading::CreateMutexW`, `windows::Win32::UI::WindowsAndMessaging::FindWindowW`, `windows::Win32::UI::WindowsAndMessaging::PostMessageW`, `windows::Win32::UI::WindowsAndMessaging::RegisterWindowMessageW`
- Produces: `pub enum SingleInstance { First(HANDLE), AlreadyRunning }`, `pub fn try_acquire() -> Result<SingleInstance, windows::core::Error>`, `pub fn notify_existing_instance()`, `pub const ALREADY_RUNNING_MSG_NAME: &str`

- [ ] **Step 1: Write failing tests for Result-based try_acquire**

Replace entire `mod tests` in `single_instance.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::CloseHandle;

    #[test]
    fn first_call_returns_first() {
        let result = test_try_acquire_with_name("Global\\HoldRect_TestMutex_FirstCall");
        match result {
            Ok(SingleInstance::First(handle)) => {
                if !handle.is_invalid() {
                    unsafe { let _ = CloseHandle(handle); }
                }
            }
            Ok(SingleInstance::AlreadyRunning) => {
                panic!("First call should return Ok(First), not Ok(AlreadyRunning)");
            }
            Err(e) => {
                panic!("First call should return Ok(First), got Err: {:?}", e);
            }
        }
    }

    #[test]
    fn second_call_returns_already_running() {
        let mutex_name = "Global\\HoldRect_TestMutex_SecondCall";
        let first_result = test_try_acquire_with_name(mutex_name);

        let handle = match first_result {
            Ok(SingleInstance::First(h)) => h,
            Ok(SingleInstance::AlreadyRunning) => {
                panic!("First call unexpectedly returned AlreadyRunning");
            }
            Err(e) => {
                panic!("First call failed: {:?}", e);
            }
        };

        let second_result = test_try_acquire_with_name(mutex_name);
        match second_result {
            Ok(SingleInstance::AlreadyRunning) => {}
            Ok(SingleInstance::First(_)) => {
                panic!("Second call should return AlreadyRunning");
            }
            Err(e) => {
                panic!("Second call failed: {:?}", e);
            }
        }

        if !handle.is_invalid() {
            unsafe { let _ = CloseHandle(handle); }
        }
    }

    #[test]
    fn different_mutex_names_independent() {
        let result1 = test_try_acquire_with_name("Global\\HoldRect_TestMutex_Ind1");
        let result2 = test_try_acquire_with_name("Global\\HoldRect_TestMutex_Ind2");

        let handle1 = match result1 {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("First mutex should return Ok(First)"),
        };
        let handle2 = match result2 {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("Second mutex should return Ok(First)"),
        };

        if !handle1.is_invalid() { unsafe { let _ = CloseHandle(handle1); } }
        if !handle2.is_invalid() { unsafe { let _ = CloseHandle(handle2); } }
    }

    #[test]
    fn mutex_released_after_handle_close() {
        let mutex_name = "Global\\HoldRect_TestMutex_Release";

        let handle = match test_try_acquire_with_name(mutex_name) {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("First acquire should succeed"),
        };

        if !handle.is_invalid() { unsafe { let _ = CloseHandle(handle); } }
        std::thread::sleep(std::time::Duration::from_millis(10));

        let handle2 = match test_try_acquire_with_name(mutex_name) {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("Should acquire after handle closed"),
        };

        if !handle2.is_invalid() { unsafe { let _ = CloseHandle(handle2); } }
    }

    fn test_try_acquire_with_name(name: &str) -> Result<SingleInstance, windows::core::Error> {
        unsafe {
            let mutex_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = CreateMutexW(None, false, windows::core::PCWSTR(mutex_name.as_ptr()))?;
            let last_error = windows::Win32::Foundation::GetLastError();
            if last_error == windows::Win32::Foundation::WIN32_ERROR(183) {
                Ok(SingleInstance::AlreadyRunning)
            } else {
                Ok(SingleInstance::First(handle))
            }
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -j1 -- single_instance
```

Expected: COMPILE ERROR — old `SingleInstance` enum without `Result` wrapper still exists

- [ ] **Step 3: Rewrite single_instance.rs (non-test part)**

Replace everything above `#[cfg(test)]` with:

```rust
//! Single instance enforcement using Windows named mutex.
//!
//! Ensures only one instance of HoldRect can run at a time.
//! Uses `FindWindow` + `PostMessage` to notify the existing instance
//! via a registered custom Windows message.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::CreateMutexW;

/// Custom Windows message name for single-instance notification.
/// Both the main instance (overlay) and second instance register this
/// via `RegisterWindowMessageW` to get the same message ID.
pub const ALREADY_RUNNING_MSG_NAME: &str = "HoldRect_AlreadyRunning";

/// Result of single instance check.
pub enum SingleInstance {
    /// This is the first instance. The mutex handle must be kept alive.
    First(HANDLE),
    /// Another instance is already running.
    AlreadyRunning,
}

/// Attempts to acquire the single-instance mutex.
///
/// Returns `Ok(First(handle))` if this is the first instance,
/// `Ok(AlreadyRunning)` if another instance is running,
/// or `Err` if mutex creation failed for an unexpected reason.
///
/// **Important**: The returned HANDLE must be kept alive for the entire
/// program lifetime, otherwise the mutex will be released.
pub fn try_acquire() -> Result<SingleInstance, windows::core::Error> {
    unsafe {
        let mutex_name: Vec<u16> = "Global\\HoldRect_SingleInstance\0"
            .encode_utf16()
            .collect();
        let handle = CreateMutexW(
            None,
            false,
            windows::core::PCWSTR(mutex_name.as_ptr()),
        )?;
        // GetLastError returns ERROR_ALREADY_EXISTS (183) if mutex already existed
        let last_error = windows::Win32::Foundation::GetLastError();
        if last_error == windows::Win32::Foundation::WIN32_ERROR(183) {
            Ok(SingleInstance::AlreadyRunning)
        } else {
            Ok(SingleInstance::First(handle))
        }
    }
}

/// Notify the existing HoldRect instance that a second instance tried to start.
///
/// Uses `FindWindow` to locate the popup window by its class name `"HoldRectPopup"`,
/// then posts a custom registered message (same name the main instance registered)
/// to trigger the "Already running" slide-in popup.
///
/// Silently exits if the window can't be found (main instance may have just closed).
pub fn notify_existing_instance() {
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, PostMessageW, RegisterWindowMessageW,
    };

    unsafe {
        let msg_name: Vec<u16> = ALREADY_RUNNING_MSG_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let msg_id = RegisterWindowMessageW(windows::core::PCWSTR(msg_name.as_ptr()));
        if msg_id == 0 {
            return;
        }

        let class_name: Vec<u16> = "HoldRectPopup\0".encode_utf16().collect();
        let hwnd = FindWindowW(
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR(std::ptr::null()),
        );
        if hwnd.is_invalid() || hwnd == Default::default() {
            return;
        }

        let _ = PostMessageW(
            hwnd,
            msg_id,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -j1 -- single_instance
```

Expected: all 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/single_instance.rs
git commit -m "refactor: rewrite single_instance with Result, PostMessage, no MessageBox"
```

---

### Task 6: Update popup WndProc to handle ALREADY_RUNNING message

**Files:**
- Modify: `src/overlay.rs`

**Interfaces:**
- Consumes: `crate::hook::send_event`, `crate::single_instance::ALREADY_RUNNING_MSG_NAME`, `RegisterWindowMessageW`
- Produces: popup WndProc that forwards `WM_HOLDRECT_ALREADY_RUNNING` to `InputEvent::InstanceAlreadyRunning`

- [ ] **Step 1: Add static for registered message ID**

At module level in `src/overlay.rs`, add:

```rust
use std::sync::atomic::{AtomicU32, Ordering};

/// Registered Windows message ID for single-instance notification.
/// Set once in `resumed()` via `RegisterWindowMessageW`.
static ALREADY_RUNNING_MSG_ID: AtomicU32 = AtomicU32::new(0);
```

- [ ] **Step 2: Register the custom message in the resumed handler**

In `fn resumed()`, inside the `#[cfg(windows)]` popup creation block, after `RegisterClassExW(&wc)`, add:

```rust
            // Register custom message for single-instance notification
            {
                let msg_name: Vec<u16> = crate::single_instance::ALREADY_RUNNING_MSG_NAME
                    .encode_utf16().chain(std::iter::once(0)).collect();
                let msg_id = unsafe {
                    RegisterWindowMessageW(windows::core::PCWSTR(msg_name.as_ptr()))
                };
                if msg_id != 0 {
                    ALREADY_RUNNING_MSG_ID.store(msg_id, Ordering::Relaxed);
                }
            }
```

- [ ] **Step 3: Update popup_wnd_proc to handle the message**

Replace the `popup_wnd_proc` function with:

```rust
            unsafe extern "system" fn popup_wnd_proc(
                hwnd: HWND,
                msg: u32,
                wparam: windows::Win32::Foundation::WPARAM,
                lparam: windows::Win32::Foundation::LPARAM,
            ) -> windows::Win32::Foundation::LRESULT {
                let registered_msg = ALREADY_RUNNING_MSG_ID.load(Ordering::Relaxed);
                if registered_msg != 0 && msg == registered_msg {
                    crate::hook::send_event(crate::state::InputEvent::InstanceAlreadyRunning);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
```

- [ ] **Step 4: Verify build**

```bash
cargo build -j1
```

Expected: BUILD SUCCEEDED

- [ ] **Step 5: Commit**

```bash
git add src/overlay.rs
git commit -m "feat: popup WndProc handles single-instance notification via registered message"
```

---

### Task 7: Update main.rs — Result-based single_instance + FirstLaunch event

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `single_instance::try_acquire() -> Result`, `single_instance::notify_existing_instance()`, `input_tx` (Sender), `InputEvent::FirstLaunch`
- Produces: main() that sends `FirstLaunch` on first start, calls `notify_existing_instance` + exits on AlreadyRunning

- [ ] **Step 1: Update single_instance check in main()**

Replace the existing `#[cfg(windows)]` single_instance block:

```rust
    #[cfg(windows)]
    let _mutex_handle: Option<windows::Win32::Foundation::HANDLE> = match crate::single_instance::try_acquire() {
        Ok(crate::single_instance::SingleInstance::First(handle)) => Some(handle),
        Ok(crate::single_instance::SingleInstance::AlreadyRunning) => {
            crate::single_instance::notify_existing_instance();
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("HoldRect: single-instance check failed: {e}, continuing anyway");
            None
        }
    };
```

- [ ] **Step 2: Send FirstLaunch event after hook listener starts**

Since `input_tx` is consumed by `start_hook_listener`, clone it first:

Change:
```rust
    let config = crate::config::AppConfig::load();
    #[cfg(windows)]
    crate::hook::start_hook_listener(input_tx, proxy, config.modifier_vk_codes);
```

To:
```rust
    let config = crate::config::AppConfig::load();
    let hook_tx = input_tx.clone();
    #[cfg(windows)]
    crate::hook::start_hook_listener(hook_tx, proxy, config.modifier_vk_codes);

    // Send FirstLaunch event for welcome popup
    let _ = input_tx.send(crate::state::InputEvent::FirstLaunch);
```

- [ ] **Step 3: Verify build**

```bash
cargo build -j1
```

Expected: BUILD SUCCEEDED

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire single_instance Result API, send FirstLaunch event on first start"
```

---

### Task 8: Handle FirstLaunch and InstanceAlreadyRunning in overlay event loop

**Files:**
- Modify: `src/overlay.rs`

**Interfaces:**
- Consumes: `InputEvent::FirstLaunch`, `InputEvent::InstanceAlreadyRunning`, `PopupManager::show_status`
- Produces: popup shown for "HoldRect started" and "Already running"

- [ ] **Step 1: Add event handling in about_to_wait input drain**

In `fn about_to_wait`, inside the `while let Ok(event) = self.input_rx.try_recv()` loop, after the existing `self.popup_manager.on_event(&event, &self.state);` line, add:

```rust
            // Handle single-instance notification events
            match &event {
                InputEvent::FirstLaunch => {
                    self.popup_manager.show_status("HoldRect started");
                }
                InputEvent::InstanceAlreadyRunning => {
                    self.popup_manager.show_status("Already running");
                }
                _ => {}
            }
```

- [ ] **Step 2: Verify build**

```bash
cargo build -j1
```

Expected: BUILD SUCCEEDED

- [ ] **Step 3: Commit**

```bash
git add src/overlay.rs
git commit -m "feat: show slide-in popup for FirstLaunch and InstanceAlreadyRunning"
```

---

### Task 9: Full test suite + manual verification

**Files:**
- No new files

- [ ] **Step 1: Run full test suite**

```bash
cargo test -j1
```

Expected: all tests pass

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -j1 -- -D warnings
```

Expected: no warnings

- [ ] **Step 3: Build release binary**

```bash
cargo build -j1 --release
```

- [ ] **Step 4: Manual test**

1. Run the exe — should see "HoldRect started" slide-in popup
2. Run it again while first is still running — second instance exits, first shows "Already running" slide-in popup

- [ ] **Step 5: Commit any fixes from manual testing**

---

### Task 10: Final review

**Files:**
- All modified files

- [ ] **Step 1: Review all changes since merge**

```bash
git log --oneline pr-3..HEAD
```

Verify:
- No MessageBox calls remain in single_instance.rs
- `try_acquire` returns `Result`
- `notify_existing_instance` uses `FindWindow` + `PostMessage`
- Popup WndProc handles registered custom message
- `FirstLaunch` and `InstanceAlreadyRunning` are in InputEvent
- No `Quick Start/` typo
- No dead code (removed `show_message`, `show_started`, `show_already_running_and_exit`)
- `hook::send_event` exists and is `pub(crate)`
