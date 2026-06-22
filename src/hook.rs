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

unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let is_key_down = w_param == WPARAM(WM_KEYDOWN as usize) || w_param == WPARAM(WM_SYSKEYDOWN as usize);

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

unsafe extern "system" fn mouse_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let ms = *(l_param.0 as *const MSLLHOOKSTRUCT);
        let msg = w_param.0 as u32;
        let pt = (ms.pt.x, ms.pt.y);
        // VK_CONTROL (0x11) catches EITHER Ctrl key via GetAsyncKeyState,
        // unlike VK_LCONTROL/VK_RCONTROL which are per-key. Intentional.
        let ctrl = GetAsyncKeyState(VK_CONTROL.0 as i32) & 0x8000u16 as i16 != 0;
        let suppress = SHOULD_SUPPRESS.load(Ordering::Relaxed);
        let drag = DRAG_IN_PROGRESS.load(Ordering::Relaxed);

        let (event, should_suppress) = decide_mouse(msg, pt, suppress, drag, ctrl);

        // Update DRAG_IN_PROGRESS based on decision.
        // Gated on `should_suppress` because decide_mouse returns true exactly
        // on state transitions (LButtonDown starts drag, LButtonUp ends it).
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

// Pure decision function — no Win32 side effects, fully unit-testable
fn decide_keyboard(vk_code: u32, is_key_down: bool) -> Option<InputEvent> {
    let is_ctrl = vk_code == VK_LCONTROL.0 as u32 || vk_code == VK_RCONTROL.0 as u32;
    if !is_ctrl {
        return None;
    }
    Some(InputEvent::ModifierChanged { pressed: is_key_down })
}

/// Pure decision function for mouse events.
/// Returns (Option<InputEvent>, should_suppress_this_event).
///
/// Suppression policy: only suppress button events (LButtonDown, LButtonUp).
/// Mouse moves pass through so the cursor stays responsive — the overlay
/// tracks position via the channel regardless.
fn decide_mouse(
    msg: u32,
    pt: (i32, i32),
    should_suppress: bool,
    drag_in_progress: bool,
    ctrl_held: bool,
) -> (Option<InputEvent>, bool) {
    // If drag in progress, track position and handle button release
    if drag_in_progress {
        if msg == WM_LBUTTONUP {
            return (Some(InputEvent::MouseButtonUp { x: pt.0, y: pt.1 }), true);
        }
        if msg == WM_MOUSEMOVE {
            // Track position but don't suppress — let cursor move freely
            return (Some(InputEvent::MouseMove { x: pt.0, y: pt.1 }), false);
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
    fn mouse_move_drag_in_progress_tracks_but_passes_through() {
        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (300, 400), false, true, false);
        assert_eq!(event, Some(InputEvent::MouseMove { x: 300, y: 400 }));
        assert!(!suppress, "MouseMove must pass through during drag so cursor stays responsive");
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

    // --- Full Ctrl+drag sequence test (simulates hook procs calling decide_* in order) ---

    #[test]
    fn full_ctrl_drag_sequence() {
        // Track shared state as the hook procs would
        let mut should_suppress: bool;
        let mut drag_in_progress = false;

        // Step 1: Ctrl pressed — keyboard hook fires
        let event = decide_keyboard(VK_LCONTROL.0 as u32, true);
        assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: true }));
        should_suppress = true; // hook proc stores this

        // Step 2: Left button down while Ctrl held, ctrl_held=true
        let ctrl = true; // GetAsyncKeyState(VK_CONTROL) would return true
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (100, 200), should_suppress, drag_in_progress, ctrl);
        assert_eq!(event, Some(InputEvent::MouseButtonDown { x: 100, y: 200 }));
        assert!(suppress, "LButtonDown should be suppressed when Ctrl+suppress active");
        drag_in_progress = true; // hook proc stores this on suppress+LButtonDown

        // Step 3: Mouse move during drag (tracks but passes through)
        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (300, 400), should_suppress, drag_in_progress, ctrl);
        assert_eq!(event, Some(InputEvent::MouseMove { x: 300, y: 400 }));
        assert!(!suppress, "MouseMove must pass through during drag so cursor stays responsive");

        // Step 4: Left button up during drag
        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (500, 600), should_suppress, drag_in_progress, ctrl);
        assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 500, y: 600 }));
        assert!(suppress, "LButtonUp should be suppressed during drag");
        drag_in_progress = false; // hook proc stores this on suppress+LButtonUp

        // Step 5: Ctrl released — keyboard hook fires
        let event = decide_keyboard(VK_LCONTROL.0 as u32, false);
        assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: false }));
        should_suppress = false; // hook proc stores this

        let _ = (&should_suppress, &drag_in_progress);
    }

    #[test]
    fn ctrl_held_mouse_moves_pass_through_before_drag() {
        // When Ctrl is held but no drag started yet, mouse moves must pass through
        let should_suppress = true;
        let drag_in_progress = false;
        let ctrl = true;

        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (100, 200), should_suppress, drag_in_progress, ctrl);
        assert_eq!(event, None, "MouseMove must NOT generate event before drag starts");
        assert!(!suppress, "MouseMove must pass through before drag starts");
    }

    #[test]
    fn ctrl_released_before_mouse_up_drag_still_suppressed() {
        // Race condition: Ctrl released before mouse button up during drag
        // Drag should still be suppressed (DRAG_IN_PROGRESS takes priority)
        let should_suppress = false; // Ctrl was released
        let drag_in_progress = true;  // but drag is still in progress
        let ctrl = false;             // Ctrl no longer held

        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (500, 600), should_suppress, drag_in_progress, ctrl);
        assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 500, y: 600 }));
        assert!(suppress, "LButtonUp must be suppressed even if Ctrl released during drag");
    }

    #[test]
    fn ctrl_released_mouse_move_during_drag_passes_through() {
        // Ctrl released mid-drag, mouse move still tracks position but passes through
        let should_suppress = false;
        let drag_in_progress = true;
        let ctrl = false;

        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (400, 500), should_suppress, drag_in_progress, ctrl);
        assert_eq!(event, Some(InputEvent::MouseMove { x: 400, y: 500 }));
        assert!(!suppress, "MouseMove must pass through during drag even if Ctrl released");
    }
}
