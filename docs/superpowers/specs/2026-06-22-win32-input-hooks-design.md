# Spec: Replace rdev with Win32 Input Hooks

## Problem

`rdev::listen` is a passive observer — it reads global input events but cannot suppress them. Two bugs result:

1. **System selection box appears**: On Windows desktop, Ctrl+LeftDrag triggers the native rubber-band selection because the mouse events reach Windows unfiltered.
2. **Foreground app interferes**: In Explorer, browsers, or any app, the native input handling runs in parallel with HoldRect's overlay — the app processes the gesture, HoldRect draws on top, both visible.

## Solution

Replace `rdev` with Win32 `SetWindowsHookExW` using `WH_KEYBOARD_LL` + `WH_MOUSE_LL`. These hooks can intercept AND suppress events before any application sees them.

## Scope

- Replace `rdev` input listener (`src/input.rs: start_input_listener`)
- Remove `start_button_poller` (mouse events now reliably captured via hook)
- Remove `LAST_POS` static (mouse hook proc receives coordinates directly)
- Remove `rdev` from `Cargo.toml`
- New module `src/hook.rs` with hook thread and suppression logic
- Wire hook into `main.rs` (replaces `start_input_listener` + `start_button_poller`)
- Update `overlay.rs` to set shared suppression flag on state transitions

Out of scope: multi-monitor, tray menu, config file, Linux/macOS.

## Architecture

```
Main thread:    winit event loop + overlay rendering (unchanged)
Hook thread:    message loop + SetWindowsHookExW callbacks
                reads SHOULD_SUPPRESS, sends InputEvent via channel
Tray thread:    tray-icon menu events (unchanged)
```

### Module: `src/hook.rs`

**Public API:**
```rust
pub fn start_hook_listener(tx: Sender<InputEvent>, proxy: EventLoopProxy<()>);
pub static SHOULD_SUPPRESS: AtomicBool;
```

`start_hook_listener` spawns a background thread that:
1. Installs `WH_KEYBOARD_LL` hook via `SetWindowsHookExW`
2. Installs `WH_MOUSE_LL` hook via `SetWindowsHookExW`
3. Runs `GetMessageW` loop (required for low-level hooks to receive callbacks)

**Hook procs** (two `extern "system"` functions):

Keyboard proc:
- `WM_KEYDOWN`/`WM_SYSKEYDOWN` for Ctrl keys → send `InputEvent::ModifierChanged { pressed: true }`, set `SHOULD_SUPPRESS = true`, call `CallNextHookEx`
- `WM_KEYUP`/`WM_SYSKEYUP` for Ctrl keys → send `InputEvent::ModifierChanged { pressed: false }`, set `SHOULD_SUPPRESS = false`, call `CallNextHookEx`
- All other keys → `CallNextHookEx` (always pass through)

Mouse proc:
- If `SHOULD_SUPPRESS == false` → `CallNextHookEx` (pass through)
- `WM_LBUTTONDOWN` + Ctrl held → send `InputEvent::MouseButtonDown { x, y }`, return 1 (suppress)
- `WM_MOUSEMOVE` + left button held → send `InputEvent::MouseMove { x, y }`, return 1 (suppress)
- `WM_LBUTTONUP` → send `InputEvent::MouseButtonUp { x, y }`, return 1 (suppress)
- All other mouse events → `CallNextHookEx` (pass through)

**Ctrl detection in mouse proc**: Check `GetAsyncKeyState(VK_CONTROL)` — simple, no state to maintain.

**Suppression rule**: Suppress only the Ctrl+LeftDrag combo. Keyboard events always pass through. Right-click, middle-click, scroll always pass through.

### Shared state: `SHOULD_SUPPRESS`

`AtomicBool` in `src/hook.rs`. Written by the overlay (main thread) on state transitions, read by the hook proc (hook thread).

Overlay sets it:
- `Idle → Armed`: `SHOULD_SUPPRESS.store(true, Ordering::Relaxed)`
- `Armed → Idle` or `Drawing → Idle`: `SHOULD_SUPPRESS.store(false, Ordering::Relaxed)`

### Changes to `overlay.rs`

- Import `SHOULD_SUPPRESS` from `crate::hook`
- In `about_to_wait`, after processing events, check if state changed and update `SHOULD_SUPPRESS` accordingly

### Changes to `main.rs`

- Remove `start_input_listener` spawn
- Remove `start_button_poller` call
- Add `start_hook_listener` call (same signature, replaces both)
- Remove `poller_tx`/`poller_proxy` variables

### Changes to `Cargo.toml`

- Remove `rdev = "0.5"` dependency

### Changes to `state.rs`

None. `InputEvent`, `DrawingState`, `AppState`, `process_event` unchanged.

## Testing Strategy

### Unit tests (in `src/hook.rs`)

Test the decision logic by calling hook proc functions directly with synthetic `KBDLLHOOKSTRUCT` / `MSLLHOOKSTRUCT` data:

1. **Keyboard hook — Ctrl press sends ModifierChanged and sets suppress flag**
2. **Keyboard hook — Ctrl release sends ModifierChanged and clears suppress flag**
3. **Keyboard hook — non-Ctrl key passes through (no event sent)**
4. **Mouse hook — LeftDown with suppress=false passes through**
5. **Mouse hook — LeftDown with suppress=true and Ctrl held → suppressed + event sent**
6. **Mouse hook — MouseMove with suppress=true and left button held → suppressed + event sent**
7. **Mouse hook — LeftUp with suppress=true → suppressed + event sent**
8. **Mouse hook — non-left button with suppress=true passes through**

To make hook procs testable: extract the decision logic into pure functions that take `(nCode, wParam, lParam, ctrl_held, suppress_flag)` and return `(Option<InputEvent>, bool_should_suppress_event)`.

### Integration test (manual)

1. Run `cargo run` 
2. Open Windows desktop → Ctrl+LeftDrag → only HoldRect red box visible, no system selection
3. Open File Explorer → Ctrl+LeftDrag → only red box, no Explorer selection
4. Open browser → Ctrl+LeftDrag → only red box, no browser text selection
5. Ctrl+C / Ctrl+V in any app still works normally
6. Normal mouse usage unaffected when Ctrl not held

## Acceptance Criteria

- [ ] Ctrl+LeftDrag on Windows desktop shows ONLY HoldRect red box (no system selection)
- [ ] Ctrl+LeftDrag in File Explorer shows ONLY HoldRect red box
- [ ] Ctrl+LeftDrag in browser shows ONLY HoldRect red box
- [ ] Normal Ctrl shortcuts (Ctrl+C, Ctrl+V, Ctrl+Z) work in all apps
- [ ] Normal mouse usage (without Ctrl) completely unaffected
- [ ] `cargo test` passes (all existing + new tests)
- [ ] `rdev` removed from Cargo.toml
- [ ] Overlay rendering unchanged (red box appears and disappears as before)
