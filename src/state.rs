/// Input events from the global listener
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    DigitPressed(u8),
    EscapePressed,
}

/// Drawing states
#[derive(Debug, Clone, PartialEq)]
pub enum DrawingState {
    /// Background idle, overlay hidden
    Idle,
    /// Ctrl held, waiting for mouse action, overlay hidden
    Armed,
    /// Mouse dragging, overlay visible, rendering rectangle
    Drawing { start: (i32, i32), current: (i32, i32) },
}

/// Application state
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub drawing: DrawingState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            drawing: DrawingState::Idle,
        }
    }
}

/// Pure state transition function. No side effects.
pub fn process_event(state: &AppState, event: &InputEvent) -> AppState {
    let drawing = match (&state.drawing, event) {
        // Idle -> Armed on modifier press
        (DrawingState::Idle, InputEvent::ModifierChanged { pressed: true }) => {
            DrawingState::Armed
        }
        // Armed -> Drawing on mouse down
        (DrawingState::Armed, InputEvent::MouseButtonDown { x, y }) => {
            DrawingState::Drawing { start: (*x, *y), current: (*x, *y) }
        }
        // Drawing: update current position on mouse move
        (DrawingState::Drawing { start, .. }, InputEvent::MouseMove { x, y }) => {
            DrawingState::Drawing { start: *start, current: (*x, *y) }
        }
        // Drawing -> Armed on mouse up (overlay hides, can re-draw)
        (DrawingState::Drawing { .. }, InputEvent::MouseButtonUp { .. }) => {
            DrawingState::Armed
        }
        // Armed -> Idle on modifier release
        (DrawingState::Armed, InputEvent::ModifierChanged { pressed: false }) => {
            DrawingState::Idle
        }
        // Drawing -> Idle on modifier release (cancel draw)
        (DrawingState::Drawing { .. }, InputEvent::ModifierChanged { pressed: false }) => {
            DrawingState::Idle
        }
        // All other combinations: no state change
        _ => state.drawing.clone(),
    };
    AppState { drawing }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Happy-path transitions ---

    #[test]
    fn idle_modifier_down_transitions_to_armed() {
        let state = AppState { drawing: DrawingState::Idle };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn armed_mouse_down_transitions_to_drawing() {
        let state = AppState { drawing: DrawingState::Armed };
        let next = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 200 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (100, 200), current: (100, 200) }
        );
    }

    #[test]
    fn drawing_mouse_move_updates_current() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (10, 20) },
        };
        let next = process_event(&state, &InputEvent::MouseMove { x: 50, y: 80 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (10, 20), current: (50, 80) }
        );
    }

    #[test]
    fn drawing_mouse_up_transitions_to_armed() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn armed_modifier_up_transitions_to_idle() {
        let state = AppState { drawing: DrawingState::Armed };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn drawing_modifier_up_transitions_to_idle() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    // --- Noop cases (illegal event for current state) ---

    #[test]
    fn idle_mouse_down_is_noop() {
        let state = AppState { drawing: DrawingState::Idle };
        let next = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn idle_mouse_move_is_noop() {
        let state = AppState { drawing: DrawingState::Idle };
        let next = process_event(&state, &InputEvent::MouseMove { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn idle_modifier_up_is_noop() {
        let state = AppState { drawing: DrawingState::Idle };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn armed_mouse_move_is_noop() {
        let state = AppState { drawing: DrawingState::Armed };
        let next = process_event(&state, &InputEvent::MouseMove { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn drawing_mouse_down_is_noop() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        };
        let next = process_event(&state, &InputEvent::MouseButtonDown { x: 99, y: 99 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (10, 20), current: (50, 80) }
        );
    }

    #[test]
    fn drawing_modifier_down_is_noop() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
        };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (10, 20), current: (50, 80) }
        );
    }

    // --- Missing catch-all coverage (untested state/event pairs) ---

    #[test]
    fn idle_mouse_button_up_is_noop() {
        let state = AppState { drawing: DrawingState::Idle };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn armed_mouse_button_up_is_noop() {
        let state = AppState { drawing: DrawingState::Armed };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    // --- Boundary values ---

    #[test]
    fn drawing_with_negative_coordinates() {
        let state = AppState { drawing: DrawingState::Armed };
        let next = process_event(&state, &InputEvent::MouseButtonDown { x: -1920, y: -1080 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (-1920, -1080), current: (-1920, -1080) }
        );
        let next = process_event(&next, &InputEvent::MouseMove { x: -100, y: -200 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (-1920, -1080), current: (-100, -200) }
        );
    }

    #[test]
    fn drawing_with_i32_boundary_values() {
        let state = AppState { drawing: DrawingState::Armed };
        let next = process_event(&state, &InputEvent::MouseButtonDown { x: i32::MAX, y: i32::MIN });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (i32::MAX, i32::MIN), current: (i32::MAX, i32::MIN) }
        );
        let next = process_event(&next, &InputEvent::MouseMove { x: i32::MIN, y: i32::MAX });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (i32::MAX, i32::MIN), current: (i32::MIN, i32::MAX) }
        );
    }

    #[test]
    fn drawing_at_origin() {
        let state = AppState { drawing: DrawingState::Armed };
        let next = process_event(&state, &InputEvent::MouseButtonDown { x: 0, y: 0 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (0, 0), current: (0, 0) }
        );
        let next = process_event(&next, &InputEvent::MouseMove { x: 0, y: 0 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (0, 0), current: (0, 0) }
        );
    }

    #[test]
    fn drawing_zero_size_mouse_up_returns_to_armed() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (100, 200), current: (100, 200) },
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    // --- Multi-event sequences ---

    #[test]
    fn multiple_mouse_moves_preserve_start() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (10, 20) },
        };
        let next = process_event(&state, &InputEvent::MouseMove { x: 30, y: 40 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (10, 20), current: (30, 40) }
        );
        let next = process_event(&next, &InputEvent::MouseMove { x: 60, y: 80 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (10, 20), current: (60, 80) }
        );
        let next = process_event(&next, &InputEvent::MouseMove { x: 5, y: 5 });
        assert_eq!(
            next.drawing,
            DrawingState::Drawing { start: (10, 20), current: (5, 5) }
        );
    }

    #[test]
    fn full_draw_lifecycle() {
        let mut state = AppState { drawing: DrawingState::Idle };

        state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        assert_eq!(state.drawing, DrawingState::Armed);

        state = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 100 });
        assert_eq!(state.drawing, DrawingState::Drawing { start: (100, 100), current: (100, 100) });

        state = process_event(&state, &InputEvent::MouseMove { x: 200, y: 150 });
        state = process_event(&state, &InputEvent::MouseMove { x: 300, y: 250 });
        assert_eq!(state.drawing, DrawingState::Drawing { start: (100, 100), current: (300, 250) });

        state = process_event(&state, &InputEvent::MouseButtonUp { x: 300, y: 250 });
        assert_eq!(state.drawing, DrawingState::Armed);

        state = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(state.drawing, DrawingState::Idle);
    }

    #[test]
    fn modifier_repress_after_release() {
        let mut state = AppState { drawing: DrawingState::Idle };

        state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        assert_eq!(state.drawing, DrawingState::Armed);

        state = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(state.drawing, DrawingState::Idle);

        state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        assert_eq!(state.drawing, DrawingState::Armed);
    }

    // --- InputEvent variant construction ---

    #[test]
    fn digit_pressed_variant_constructs() {
        let event = InputEvent::DigitPressed(1);
        assert_eq!(event, InputEvent::DigitPressed(1));
    }

    #[test]
    fn escape_pressed_variant_constructs() {
        let event = InputEvent::EscapePressed;
        assert_eq!(event, InputEvent::EscapePressed);
    }

    // --- Trait coverage ---

    #[test]
    fn default_app_state_is_idle() {
        let state = AppState::default();
        assert_eq!(state.drawing, DrawingState::Idle);
    }

    #[test]
    fn app_state_clone_independence() {
        let original = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (30, 40) },
        };
        let mut cloned = original.clone();
        cloned.drawing = DrawingState::Idle;
        assert_eq!(original.drawing, DrawingState::Drawing { start: (10, 20), current: (30, 40) });
        assert_eq!(cloned.drawing, DrawingState::Idle);
    }
}
