use crate::state::InputEvent;
use windows::Win32::UI::Input::KeyboardAndMouse::*;


// Pure decision function — no Win32 side effects, fully unit-testable
fn decide_keyboard(vk_code: u32, is_key_down: bool) -> Option<InputEvent> {
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
