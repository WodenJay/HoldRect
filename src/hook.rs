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
static MODIFIER_CODES: OnceLock<Vec<u32>> = OnceLock::new();

/// Start global input hook listener on a background thread.
/// Replaces rdev — intercepts and suppresses modifier+mouse combo.
pub fn start_hook_listener(tx: Sender<InputEvent>, proxy: EventLoopProxy<()>, modifier_codes: Vec<u32>) {
    TX.set(tx).expect("start_hook_listener called twice");
    PROXY.set(proxy).expect("start_hook_listener called twice");
    MODIFIER_CODES.set(modifier_codes).expect("start_hook_listener called twice");

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

        if let Some(event) = decide_keyboard(kb.vkCode, is_key_down, MODIFIER_CODES.get().expect("MODIFIER_CODES not set")) {
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
        // SHOULD_SUPPRESS (keyboard hook) is the single source of truth for
        // Alt-held state. GetAsyncKeyState(VK_MENU) is unreliable inside
        // WH_MOUSE_LL callbacks in browsers/terminals.
        let suppress = SHOULD_SUPPRESS.load(Ordering::Relaxed);
        let modifier_held = suppress;
        let drag = DRAG_IN_PROGRESS.load(Ordering::Relaxed);

        let (event, should_suppress) = decide_mouse(msg, pt, suppress, drag, modifier_held);

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
pub(crate) fn decide_keyboard(vk_code: u32, is_key_down: bool, modifier_codes: &[u32]) -> Option<InputEvent> {
    if !modifier_codes.contains(&vk_code) {
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
pub(crate) fn decide_mouse(
    msg: u32,
    pt: (i32, i32),
    should_suppress: bool,
    drag_in_progress: bool,
    modifier_held: bool,
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

    // No drag in progress: only act if suppress mode active and modifier held
    if !should_suppress || !modifier_held {
        return (None, false);
    }

    // should_suppress && modifier_held
    if msg == WM_LBUTTONDOWN {
        return (Some(InputEvent::MouseButtonDown { x: pt.0, y: pt.1 }), true);
    }

    (None, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alt_down_returns_modifier_pressed() {
        let result = decide_keyboard(VK_LMENU.0 as u32, true, &[0x12, 0xA4, 0xA5]);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn alt_up_returns_modifier_released() {
        let result = decide_keyboard(VK_LMENU.0 as u32, false, &[0x12, 0xA4, 0xA5]);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: false }));
    }

    #[test]
    fn right_alt_down_returns_modifier_pressed() {
        let result = decide_keyboard(VK_RMENU.0 as u32, true, &[0x12, 0xA4, 0xA5]);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn generic_alt_down_returns_modifier_pressed() {
        let result = decide_keyboard(VK_MENU.0 as u32, true, &[0x12, 0xA4, 0xA5]);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn generic_alt_up_returns_modifier_released() {
        let result = decide_keyboard(VK_MENU.0 as u32, false, &[0x12, 0xA4, 0xA5]);
        assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: false }));
    }

    #[test]
    fn non_alt_key_returns_none() {
        let result = decide_keyboard(VK_LSHIFT.0 as u32, true, &[0x12, 0xA4, 0xA5]);
        assert_eq!(result, None);
    }

    #[test]
    fn non_alt_key_up_returns_none() {
        let result = decide_keyboard(0x41, false, &[0x12, 0xA4, 0xA5]); // 'A' key
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

    // --- Full Alt+drag sequence test (simulates hook procs calling decide_* in order) ---

    #[test]
    fn full_alt_drag_sequence() {
        // Track shared state as the hook procs would
        let mut should_suppress: bool;
        let mut drag_in_progress = false;

        // Step 1: Alt pressed — keyboard hook fires
        let alt_codes: &[u32] = &[0x12, 0xA4, 0xA5];
        let event = decide_keyboard(VK_LMENU.0 as u32, true, alt_codes);
        assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: true }));
        should_suppress = true; // hook proc stores this

        // Step 2: Left button down while Alt held, alt_held=true
        let alt = true; // GetAsyncKeyState(VK_MENU) would return true
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (100, 200), should_suppress, drag_in_progress, alt);
        assert_eq!(event, Some(InputEvent::MouseButtonDown { x: 100, y: 200 }));
        assert!(suppress, "LButtonDown should be suppressed when Alt+suppress active");
        drag_in_progress = true; // hook proc stores this on suppress+LButtonDown

        // Step 3: Mouse move during drag (tracks but passes through)
        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (300, 400), should_suppress, drag_in_progress, alt);
        assert_eq!(event, Some(InputEvent::MouseMove { x: 300, y: 400 }));
        assert!(!suppress, "MouseMove must pass through during drag so cursor stays responsive");

        // Step 4: Left button up during drag
        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (500, 600), should_suppress, drag_in_progress, alt);
        assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 500, y: 600 }));
        assert!(suppress, "LButtonUp should be suppressed during drag");
        drag_in_progress = false; // hook proc stores this on suppress+LButtonUp

        // Step 5: Alt released — keyboard hook fires
        let event = decide_keyboard(VK_LMENU.0 as u32, false, alt_codes);
        assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: false }));
        should_suppress = false; // hook proc stores this

        let _ = (&should_suppress, &drag_in_progress);
    }

    #[test]
    fn alt_held_mouse_moves_pass_through_before_drag() {
        // When Alt is held but no drag started yet, mouse moves must pass through
        let should_suppress = true;
        let drag_in_progress = false;
        let alt = true;

        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (100, 200), should_suppress, drag_in_progress, alt);
        assert_eq!(event, None, "MouseMove must NOT generate event before drag starts");
        assert!(!suppress, "MouseMove must pass through before drag starts");
    }

    #[test]
    fn alt_released_before_mouse_up_drag_still_suppressed() {
        // Race condition: Alt released before mouse button up during drag
        // Drag should still be suppressed (DRAG_IN_PROGRESS takes priority)
        let should_suppress = false; // Alt was released
        let drag_in_progress = true;  // but drag is still in progress
        let alt = false;              // Alt no longer held

        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (500, 600), should_suppress, drag_in_progress, alt);
        assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 500, y: 600 }));
        assert!(suppress, "LButtonUp must be suppressed even if Alt released during drag");
    }

    #[test]
    fn alt_released_mouse_move_during_drag_passes_through() {
        // Alt released mid-drag, mouse move still tracks position but passes through
        let should_suppress = false;
        let drag_in_progress = true;
        let alt_held = false;

        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (400, 500), should_suppress, drag_in_progress, alt_held);
        assert_eq!(event, Some(InputEvent::MouseMove { x: 400, y: 500 }));
        assert!(!suppress, "MouseMove must pass through during drag even if Ctrl released");
    }

    // --- Configurable modifier key tests (3-param decide_keyboard) ---

    #[test]
    fn alt_key_with_alt_config_detected() {
        let codes = vec![0x12, 0xA4, 0xA5];
        assert_eq!(decide_keyboard(VK_LMENU.0 as u32, true, &codes), Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn ctrl_key_with_ctrl_config_detected() {
        let codes = vec![0x11, 0xA2, 0xA3];
        assert_eq!(decide_keyboard(VK_LCONTROL.0 as u32, true, &codes), Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn shift_key_with_shift_config_detected() {
        let codes = vec![0x10, 0xA0, 0xA1];
        assert_eq!(decide_keyboard(VK_LSHIFT.0 as u32, true, &codes), Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn win_key_with_win_config_detected() {
        let codes = vec![0x5B, 0x5C];
        assert_eq!(decide_keyboard(VK_LWIN.0 as u32, true, &codes), Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn non_modifier_ignored_with_alt_config() {
        let codes = vec![0x12, 0xA4, 0xA5];
        assert_eq!(decide_keyboard(0x41, true, &codes), None); // 'A' key
    }

    #[test]
    fn alt_key_not_detected_with_ctrl_config() {
        let codes = vec![0x11, 0xA2, 0xA3]; // Ctrl config
        assert_eq!(decide_keyboard(VK_LMENU.0 as u32, true, &codes), None);
    }

    // -- decide_keyboard edge cases --

    #[test]
    fn decide_keyboard_empty_modifier_codes_returns_none() {
        let codes: Vec<u32> = vec![];
        assert_eq!(decide_keyboard(0x12, true, &codes), None);
    }

    #[test]
    fn decide_keyboard_zero_vk_code_not_in_codes() {
        let codes = vec![0x12, 0xA4, 0xA5];
        assert_eq!(decide_keyboard(0, true, &codes), None);
    }

    #[test]
    fn decide_keyboard_zero_vk_code_in_codes() {
        let codes = vec![0, 0x12];
        assert_eq!(decide_keyboard(0, true, &codes), Some(InputEvent::ModifierChanged { pressed: true }));
    }

    #[test]
    fn decide_keyboard_max_u32_not_in_codes() {
        let codes = vec![0x12, 0xA4, 0xA5];
        assert_eq!(decide_keyboard(u32::MAX, true, &codes), None);
    }

    // -- decide_mouse edge cases --

    #[test]
    fn mouse_drag_in_progress_with_suppress_flag_true_move_still_passes_through() {
        // Even if suppress=true externally, drag move should pass through
        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (300, 400), true, true, true);
        assert_eq!(event, Some(InputEvent::MouseMove { x: 300, y: 400 }));
        assert!(!suppress, "MouseMove must pass through during drag regardless of suppress state");
    }

    #[test]
    fn mouse_lbuttondown_during_drag_is_noop() {
        // Repeated LButtonDown while drag already in progress
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (100, 200), true, true, true);
        assert_eq!(event, None, "LButtonDown during active drag should be ignored");
        assert!(!suppress, "LButtonDown during active drag should not suppress");
    }

    #[test]
    fn mouse_rbuttondown_during_drag_is_noop() {
        let (event, suppress) = decide_mouse(WM_RBUTTONDOWN, (100, 200), false, true, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_rbuttonup_during_drag_is_noop() {
        let (event, suppress) = decide_mouse(WM_RBUTTONUP, (100, 200), false, true, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_mbuttondown_during_drag_is_noop() {
        let (event, suppress) = decide_mouse(WM_MBUTTONDOWN, (100, 200), false, true, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_mbuttonup_during_drag_is_noop() {
        let (event, suppress) = decide_mouse(WM_MBUTTONUP, (100, 200), false, true, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_unknown_message_during_drag_is_noop() {
        // Use a non-standard WM_ message code
        let (event, suppress) = decide_mouse(WM_MOUSEWHEEL, (100, 200), false, true, false);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_unknown_message_outside_drag_suppress_modifier_held_is_noop() {
        // Not in drag, suppress+modifier_held both true, but message is not LBUTTONDOWN
        let (event, suppress) = decide_mouse(WM_MOUSEWHEEL, (100, 200), true, false, true);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    #[test]
    fn mouse_move_suppress_true_modifier_held_true_no_drag_is_noop() {
        // Mouse move outside drag should not generate events
        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (100, 200), true, false, true);
        assert_eq!(event, None);
        assert!(!suppress);
    }

    // -- Full sequence: multiple drags in one Alt-hold session --

    #[test]
    fn multiple_drags_in_single_alt_hold() {
        let mut should_suppress = false;
        let mut drag_in_progress = false;
        let alt_codes: &[u32] = &[0x12, 0xA4, 0xA5];

        // Alt down
        let event = decide_keyboard(VK_LMENU.0 as u32, true, alt_codes);
        assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: true }));
        should_suppress = true;

        // Drag 1: down -> move -> up
        let alt = true;
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (10, 10), should_suppress, drag_in_progress, alt);
        assert!(event.is_some());
        assert!(suppress);
        drag_in_progress = true;

        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (50, 50), should_suppress, drag_in_progress, alt);
        assert!(event.is_some());
        assert!(!suppress);

        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (50, 50), should_suppress, drag_in_progress, alt);
        assert!(event.is_some());
        assert!(suppress);
        drag_in_progress = false;

        // Drag 2: down -> move -> up (Alt still held)
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (20, 20), should_suppress, drag_in_progress, alt);
        assert!(event.is_some());
        assert!(suppress);
        drag_in_progress = true;

        let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (80, 80), should_suppress, drag_in_progress, alt);
        assert!(event.is_some());
        assert!(!suppress);

        let (event, suppress) = decide_mouse(WM_LBUTTONUP, (80, 80), should_suppress, drag_in_progress, alt);
        assert!(event.is_some());
        assert!(suppress);
        drag_in_progress = false;

        // Alt up
        let event = decide_keyboard(VK_LMENU.0 as u32, false, alt_codes);
        assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: false }));
        should_suppress = false;

        let _ = (&should_suppress, &drag_in_progress);
    }

    #[test]
    fn drag_with_negative_coordinates() {
        let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (-100, -200), true, false, true);
        assert_eq!(event, Some(InputEvent::MouseButtonDown { x: -100, y: -200 }));
        assert!(suppress);
    }

    #[test]
    fn drag_move_with_max_coordinates() {
        let (event, _) = decide_mouse(WM_MOUSEMOVE, (i32::MAX, i32::MIN), false, true, false);
        assert_eq!(event, Some(InputEvent::MouseMove { x: i32::MAX, y: i32::MIN }));
    }
}
