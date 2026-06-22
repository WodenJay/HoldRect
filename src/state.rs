/// Input events from the global listener
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    ModifierChanged { pressed: bool },
    MouseButtonDown { x: i32, y: i32 },
    MouseButtonUp { x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
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
}
