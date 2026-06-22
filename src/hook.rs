use crate::state::InputEvent;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;


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
fn decide_mouse(
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
}
