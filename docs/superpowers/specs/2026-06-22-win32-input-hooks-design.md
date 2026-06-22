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
- Remove `LAST_POS` static (mouse hook proc receives coordinates directly via `MSLLHOOKSTRUCT.pt`)
- Remove `rdev` from `Cargo.toml`
- New module `src/hook.rs` (gated with `#[cfg(windows)]`) with hook thread and suppression logic
- Wire hook into `main.rs` (replaces `start_input_listener` + `start_button_poller`)
- No changes to `overlay.rs` or `state.rs`

Out of scope: multi-monitor, tray menu, config file, Linux/macOS (Linux/macOS will use platform-specific hooks in v0.3).

## Architecture

```
Main thread:    winit event loop + overlay rendering (unchanged)
Hook thread:    message loop + SetWindowsHookExW callbacks
                sole authority on SHOULD_SUPPRESS, sends InputEvent via channel
                wakes main event loop via proxy.send_event(()) after each tx.send()
Tray thread:    tray-icon menu events (unchanged)
```

### Module: `src/hook.rs` (`#[cfg(windows)]`)

**Public API:**
```rust
pub fn start_hook_listener(tx: Sender<InputEvent>, proxy: EventLoopProxy<()>);
```

`start_hook_listener` spawns a background thread that:
1. Installs `WH_KEYBOARD_LL` hook via `SetWindowsHookExW`
2. Installs `WH_MOUSE_LL` hook via `SetWindowsHookExW`
3. Runs `GetMessageW` loop (required for low-level hooks to receive callbacks)
4. After each `tx.send(event)`, calls `proxy.send_event(())` to wake the winit event loop (same pattern as current `start_input_listener`)

**Internal state** (file-level statics, not pub):
- `SHOULD_SUPPRESS: AtomicBool` — true when Ctrl is held (set by keyboard proc)
- `DRAG_IN_PROGRESS: AtomicBool` — true when a LeftDown was suppressed (set by mouse proc)
- `TX: OnceLock<Sender<InputEvent>>` — channel sender, set once in `start_hook_listener`
- `PROXY: OnceLock<EventLoopProxy<()>>` — event loop proxy, set once in `start_hook_listener`

**Sole authority on suppression**: `SHOULD_SUPPRESS` is written ONLY by the keyboard hook proc. The overlay does NOT write to it. This eliminates dual-authorship races.

**Hook procs** (two `extern "system"` functions):

Keyboard proc:
- `WM_KEYDOWN`/`WM_SYSKEYDOWN` for Ctrl keys (VK_LCONTROL / VK_RCONTROL):
  → send `InputEvent::ModifierChanged { pressed: true }`, set `SHOULD_SUPPRESS = true`
  → always call `CallNextHookEx` (keyboard events always pass through)
- `WM_KEYUP`/`WM_SYSKEYUP` for Ctrl keys:
  → send `InputEvent::ModifierChanged { pressed: false }`, set `SHOULD_SUPPRESS = false`
  → always call `CallNextHookEx`
- All other keys → `CallNextHookEx` (pass through)

Mouse proc (priority order — drag_in_progress checks come first, SHOULD_SUPPRESS only gates new drag initiation):
- If `DRAG_IN_PROGRESS == true` AND `WM_LBUTTONUP` → send `InputEvent::MouseButtonUp`, set `DRAG_IN_PROGRESS = false`, return 1 (suppress — ensures LeftUp is always suppressed once drag started, even if Ctrl released first)
- If `DRAG_IN_PROGRESS == true` AND `WM_MOUSEMOVE` → send `InputEvent::MouseMove { x, y }`, return 1 (suppress — continue tracking active drag regardless of Ctrl state)
- If `SHOULD_SUPPRESS == false` → `CallNextHookEx` (pass through — no drag in progress, Ctrl not held)
- `WM_LBUTTONDOWN` + Ctrl held (`GetAsyncKeyState(VK_CONTROL)`) → send `InputEvent::MouseButtonDown { x, y }`, set `DRAG_IN_PROGRESS = true`, return 1 (suppress — start new drag)
- All other mouse events → `CallNextHookEx` (pass through)

**Race condition fix**: `DRAG_IN_PROGRESS` ensures `WM_LBUTTONUP` is always suppressed if the preceding `WM_LBUTTONDOWN` was suppressed — even if Ctrl is released before the mouse button (which sets `SHOULD_SUPPRESS = false`). This prevents the foreground app from receiving an orphaned `WM_LBUTTONUP`.

**Coordinates**: `MSLLHOOKSTRUCT.pt` provides screen coordinates as `LONG` (i32), same coordinate space as the current rdev→i32 cast. No DPI handling change needed — both use raw screen pixels.

**Suppression rule**: Suppress only the Ctrl+LeftDrag combo. Keyboard events always pass through. Right-click, middle-click, scroll always pass through.

**Memory ordering**: `Ordering::Relaxed` is used for `SHOULD_SUPPRESS` and `DRAG_IN_PROGRESS`. Safe on x86 (strong memory model guarantees visibility). If porting to ARM in v0.3, upgrade to `Ordering::Acquire`/`Release`.

### Pure decision functions (for testability)

Extract the hook proc decision logic into pure functions:

```rust
// Keyboard: returns Some(InputEvent) if key is Ctrl
fn decide_keyboard(vk_code: u32, is_key_down: bool) -> Option<InputEvent>;

// Mouse: returns (Option<InputEvent>, suppress: bool)
fn decide_mouse(
    msg: u32,              // WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_LBUTTONUP
    pt: (i32, i32),        // screen coordinates
    should_suppress: bool,  // current SHOULD_SUPPRESS value
    drag_in_progress: bool, // current DRAG_IN_PROGRESS value
    ctrl_held: bool,        // GetAsyncKeyState(VK_CONTROL)
) -> (Option<InputEvent>, bool);
```

The actual `extern "system"` hook procs read the Win32 structs and call these functions, then act on the return values. This keeps all decision logic testable without Win32 types.

### Changes to `main.rs`

- Remove `start_input_listener` spawn (and its thread::spawn block)
- Remove `start_button_poller` call
- Remove `poller_tx`/`poller_proxy` variables
- Add `use crate::hook::start_hook_listener;` (behind `#[cfg(windows)]`)
- Replace with `start_hook_listener(input_tx, proxy);` (behind `#[cfg(windows)]`)

### Changes to `Cargo.toml`

- Remove `rdev = "0.5"`

### Changes to `state.rs`

None. `InputEvent`, `DrawingState`, `AppState`, `process_event` unchanged.

### Changes to `overlay.rs`

None. The overlay does not write to `SHOULD_SUPPRESS`.

## Testing Strategy

### Unit tests (in `src/hook.rs`)

Test the pure decision functions directly. No Win32 types needed — just `u32` msg codes and `(i32, i32)` coordinates.

**`decide_keyboard` tests:**
1. Ctrl (VK_LCONTROL) key down → returns `Some(ModifierChanged { pressed: true })`
2. Ctrl (VK_LCONTROL) key up → returns `Some(ModifierChanged { pressed: false })`
3. Non-Ctrl key down → returns `None`
4. Non-Ctrl key up → returns `None`

**`decide_mouse` tests:**

| # | Test | should_suppress | drag_in_progress | ctrl_held | msg | Expected |
|---|------|----------------|-----------------|-----------|-----|----------|
| 5 | LeftDown, no suppress | false | false | false | WM_LBUTTONDOWN | (None, false) — pass through |
| 6 | LeftDown, suppress, Ctrl held | true | false | true | WM_LBUTTONDOWN | (Some(MouseButtonDown), true) — suppress |
| 7 | LeftDown, suppress, Ctrl NOT held | true | false | false | WM_LBUTTONDOWN | (None, false) — pass through |
| 8 | MouseMove, drag in progress | false | true | false | WM_MOUSEMOVE | (Some(MouseMove), true) — suppress |
| 9 | MouseMove, no drag | true | false | true | WM_MOUSEMOVE | (None, false) — pass through |
| 10 | LeftUp, drag in progress | false | true | false | WM_LBUTTONUP | (Some(MouseButtonUp), true) — suppress, clear drag |
| 11 | LeftUp, suppress but no drag | true | false | true | WM_LBUTTONUP | (None, false) — pass through |
| 12 | RightDown, suppress | true | false | true | WM_RBUTTONDOWN | (None, false) — pass through |

### Integration test (manual)

1. `cargo run`
2. Windows desktop → Ctrl+LeftDrag → only HoldRect red box visible, no system selection
3. File Explorer → Ctrl+LeftDrag → only red box, no Explorer selection
4. Browser → Ctrl+LeftDrag → only red box, no browser text selection
5. Ctrl+C / Ctrl+V / Ctrl+Z in any app still works
6. Normal mouse usage (no Ctrl) completely unaffected
7. Ctrl+LeftDrag, release Ctrl before releasing mouse → no orphaned LeftUp reaches foreground app

## Acceptance Criteria

- [ ] Ctrl+LeftDrag on Windows desktop shows ONLY HoldRect red box (no system selection)
- [ ] Ctrl+LeftDrag in File Explorer shows ONLY HoldRect red box
- [ ] Ctrl+LeftDrag in browser shows ONLY HoldRect red box
- [ ] Normal Ctrl shortcuts (Ctrl+C, Ctrl+V, Ctrl+Z) work in all apps
- [ ] Normal mouse usage (without Ctrl) completely unaffected
- [ ] `cargo test` passes (all existing + new tests)
- [ ] `rdev` removed from Cargo.toml
- [ ] Overlay rendering unchanged (red box appears and disappears as before)
