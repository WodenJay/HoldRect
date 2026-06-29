# Single Instance + Popup Integration Design

## Context

PR #3 adds single-instance enforcement via Windows named mutex. The implementation has several issues:
- Uses MessageBox for notifications (inconsistent with app's slide-in popup style)
- `CreateMutexW` error handling silently swallows errors (returns default HANDLE)
- The entire PR is mixed with cargo fmt noise (~70% of diff)
- README typo: `Quick Start/` (trailing slash)

## Design

### 1. Undo cargo fmt noise

Strategy: Cherry-pick only the functional changes from PR #3 onto a clean branch.
The functional commits are: single-instance, magnifier flicker fix, toggle mode, digit key suppression.

### 2. Replace MessageBox with PopupManager slide-in popups

Two scenarios:

**First launch ("HoldRect started")**
- After event loop starts, send `InputEvent::FirstLaunch` through the input channel
- Overlay receives it, calls `popup_manager.show_status("HoldRect started")`
- Slide-in popup appears, auto-dismisses after HOLD_DURATION_MS (1s)

**Second launch ("Already running")**
- Second instance detects `AlreadyRunning` from `try_acquire()`
- Calls `notify_existing_instance()` which:
  1. `FindWindow("HoldRectMain")` to find the main overlay window
  2. `PostMessage(hwnd, WM_HOLDRECT_ALREADY_RUNNING, 0, 0)` to notify
  3. `exit(0)` — second instance exits silently
- Main process overlay WndProc receives `WM_HOLDRECT_ALREADY_RUNNING`
- Forwards as `InputEvent::InstanceAlreadyRunning` through input_tx
- Overlay calls `popup_manager.show_status("Already running")`

### 3. Error handling

- `try_acquire()` returns `Result<SingleInstance, windows::core::Error>`
- `CreateMutexW` fails with non-`ERROR_ALREADY_EXISTS` → `Err(e)`
- `main()` handles `Err` with `eprintln!` + continue (don't block startup)
- `notify_existing_instance()` silently exits if `FindWindow` fails (main may have just closed)

### 4. Custom Windows message

Define `WM_HOLDRECT_ALREADY_RUNNING = WM_USER + 0x100` in overlay module.
Register the overlay window class with name `"HoldRectMain"` so `FindWindow` can locate it.

### 5. InputEvent additions

```rust
pub enum InputEvent {
    // ... existing variants ...
    FirstLaunch,              // first instance started, show welcome popup
    InstanceAlreadyRunning,   // another instance tried to start, show "already running"
}
```

Both are no-ops in `process_event()` — they don't change drawing/pinned/magnifier state.
They only trigger popup display in the overlay event handler.

### 6. Window class name

The overlay window already has title `"HoldRect"`. We'll use a dedicated class name
`"HoldRectMain"` registered in `set_click_through` (or the window creation path) so
`FindWindow` reliably finds it.

## Files to modify

| File | Change |
|------|--------|
| `src/single_instance.rs` | Remove MessageBox functions, `try_acquire` returns `Result`, add `notify_existing_instance()` with PostMessage |
| `src/main.rs` | Handle `try_acquire` Result, send `FirstLaunch` event on first start, call `notify_existing_instance` + exit on AlreadyRunning |
| `src/state.rs` | Add `FirstLaunch` and `InstanceAlreadyRunning` to `InputEvent` |
| `src/overlay.rs` | Handle `FirstLaunch`/`InstanceAlreadyRunning` → popup_manager.show_status(); register custom window message + WndProc for WM_HOLDRECT_ALREADY_RUNNING |
| `src/hook.rs` | Keep digit key suppression and flicker fix (functional changes from PR) |
| `src/magnifier.rs` | Keep flicker fix and toggle mode (functional changes from PR) |
| `README.md` | Fix `Quick Start/` typo |
| Other files | Only clippy fixes (BI_RGB.0, too_many_arguments allow), no cargo fmt noise |

## Testing

- `single_instance::try_acquire` tests updated for `Result` return type
- `process_event` tests: `FirstLaunch` and `InstanceAlreadyRunning` are no-ops on state
- Manual test: launch twice, verify slide-in popup on both launches
