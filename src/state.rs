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

/// A pinned rectangle with per-rect flags
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub spotlight: bool,
}

/// Application state
#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub drawing: DrawingState,
    pub pinned_rects: Vec<PinnedRect>,
    pub pinned_active: bool,
    pub spotlight_active: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            drawing: DrawingState::Idle,
            pinned_rects: Vec::new(),
            pinned_active: false,
            spotlight_active: false,
        }
    }
}

pub(crate) fn normalize_rect(start: (i32, i32), current: (i32, i32)) -> (i32, i32, i32, i32) {
    let x0 = start.0.min(current.0);
    let y0 = start.1.min(current.1);
    let x1 = start.0.max(current.0);
    let y1 = start.1.max(current.1);
    (x0, y0, x1, y1)
}

/// Pure state transition function. No side effects.
pub fn process_event(state: &AppState, event: &InputEvent) -> AppState {
    let (drawing, pinned_active, spotlight_active, pinned_rects) = match (&state.drawing, event) {
        // --- DigitPressed(1) toggle (only in Armed or Drawing, i.e. modifier held) ---
        (DrawingState::Armed, InputEvent::DigitPressed(1)) => {
            (state.drawing.clone(), !state.pinned_active, state.spotlight_active, state.pinned_rects.clone())
        }
        (DrawingState::Drawing { .. }, InputEvent::DigitPressed(1)) => {
            (state.drawing.clone(), !state.pinned_active, state.spotlight_active, state.pinned_rects.clone())
        }

        // --- DigitPressed(2) spotlight toggle (only in Armed or Drawing) ---
        (DrawingState::Armed, InputEvent::DigitPressed(2)) => {
            (state.drawing.clone(), state.pinned_active, !state.spotlight_active, state.pinned_rects.clone())
        }
        (DrawingState::Drawing { .. }, InputEvent::DigitPressed(2)) => {
            (state.drawing.clone(), state.pinned_active, !state.spotlight_active, state.pinned_rects.clone())
        }

        // --- EscapePressed: clear all pinned rects and reset flags ---
        (_, InputEvent::EscapePressed) => {
            let drawing = match &state.drawing {
                DrawingState::Drawing { .. } => DrawingState::Armed,
                other => other.clone(),
            };
            (drawing, false, false, Vec::new())
        }

        // --- Existing transitions (pinned_active/pinned_rects unchanged unless noted) ---

        // Idle -> Armed on modifier press
        (DrawingState::Idle, InputEvent::ModifierChanged { pressed: true }) => {
            (DrawingState::Armed, state.pinned_active, state.spotlight_active, state.pinned_rects.clone())
        }
        // Armed -> Drawing on mouse down
        (DrawingState::Armed, InputEvent::MouseButtonDown { x, y }) => {
            (DrawingState::Drawing { start: (*x, *y), current: (*x, *y) }, state.pinned_active, state.spotlight_active, state.pinned_rects.clone())
        }
        // Drawing: update current position on mouse move
        (DrawingState::Drawing { start, .. }, InputEvent::MouseMove { x, y }) => {
            (DrawingState::Drawing { start: *start, current: (*x, *y) }, state.pinned_active, state.spotlight_active, state.pinned_rects.clone())
        }
        // Drawing -> Armed on mouse up
        (DrawingState::Drawing { start, .. }, InputEvent::MouseButtonUp { x, y }) => {
            let final_current = (*x, *y);
            let mut rects = state.pinned_rects.clone();
            if state.pinned_active {
                let (x0, y0, x1, y1) = normalize_rect(*start, final_current);
                rects.push(PinnedRect { x0, y0, x1, y1, spotlight: state.spotlight_active });
            }
            (DrawingState::Armed, false, false, rects)
        }
        // Armed -> Idle on modifier release
        (DrawingState::Armed, InputEvent::ModifierChanged { pressed: false }) => {
            (DrawingState::Idle, false, false, state.pinned_rects.clone())
        }
        // Drawing -> Idle on modifier release
        (DrawingState::Drawing { start, current }, InputEvent::ModifierChanged { pressed: false }) => {
            let mut rects = state.pinned_rects.clone();
            if state.pinned_active {
                let (x0, y0, x1, y1) = normalize_rect(*start, *current);
                rects.push(PinnedRect { x0, y0, x1, y1, spotlight: state.spotlight_active });
            }
            (DrawingState::Idle, false, false, rects)
        }
        // All other combinations: no state change
        _ => (state.drawing.clone(), state.pinned_active, state.spotlight_active, state.pinned_rects.clone()),
    };
    AppState { drawing, pinned_rects, pinned_active, spotlight_active }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Happy-path transitions ---

    #[test]
    fn idle_modifier_down_transitions_to_armed() {
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn armed_mouse_down_transitions_to_drawing() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
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
            ..Default::default()
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
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn armed_modifier_up_transitions_to_idle() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn drawing_modifier_up_transitions_to_idle() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    // --- Noop cases (illegal event for current state) ---

    #[test]
    fn idle_mouse_down_is_noop() {
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        let next = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn idle_mouse_move_is_noop() {
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        let next = process_event(&state, &InputEvent::MouseMove { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn idle_modifier_up_is_noop() {
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn armed_mouse_move_is_noop() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::MouseMove { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn drawing_mouse_down_is_noop() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            ..Default::default()
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
            ..Default::default()
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
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn armed_mouse_button_up_is_noop() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    // --- Boundary values ---

    #[test]
    fn drawing_with_negative_coordinates() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
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
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
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
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
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
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 100, y: 200 });
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    // --- Multi-event sequences ---

    #[test]
    fn multiple_mouse_moves_preserve_start() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (10, 20) },
            ..Default::default()
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
        let mut state = AppState { drawing: DrawingState::Idle, ..Default::default() };

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
        let mut state = AppState { drawing: DrawingState::Idle, ..Default::default() };

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
            ..Default::default()
        };
        let mut cloned = original.clone();
        cloned.drawing = DrawingState::Idle;
        assert_eq!(original.drawing, DrawingState::Drawing { start: (10, 20), current: (30, 40) });
        assert_eq!(cloned.drawing, DrawingState::Idle);
    }

    #[test]
    fn default_app_state_has_empty_pinned() {
        let state = AppState::default();
        assert!(state.pinned_rects.is_empty());
        assert!(!state.pinned_active);
    }

    // --- Pinned mode: DigitPressed toggle ---

    #[test]
    fn armed_digit_1_toggles_pinned_active() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(1));
        assert!(next.pinned_active);
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn armed_digit_1_toggle_off() {
        let state = AppState { drawing: DrawingState::Armed, pinned_active: true, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(1));
        assert!(!next.pinned_active);
    }

    #[test]
    fn drawing_digit_1_toggles_pinned_active() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::DigitPressed(1));
        assert!(next.pinned_active);
        assert_eq!(next.drawing, DrawingState::Drawing { start: (10, 20), current: (50, 80) });
    }

    #[test]
    fn idle_digit_1_is_noop() {
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(1));
        assert!(!next.pinned_active);
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn digit_2_does_not_toggle_pinned_but_toggles_spotlight() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(2));
        assert!(!next.pinned_active, "digit 2 must not affect pinned_active");
        assert!(next.spotlight_active, "digit 2 toggles spotlight_active on");
    }

    // --- Pinned mode: mouse up with pinned_active ---

    #[test]
    fn drawing_mouse_up_pinned_pushes_rect() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert_eq!(next.drawing, DrawingState::Armed);
        assert_eq!(next.pinned_rects, vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: false }]);
        assert!(!next.pinned_active, "pinned_active resets after mouse up");
    }

    #[test]
    fn drawing_mouse_up_pinned_normalizes_rect() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (50, 80), current: (10, 20) },
            pinned_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 10, y: 20 });
        assert_eq!(next.pinned_rects, vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: false }]);
    }

    #[test]
    fn drawing_mouse_up_not_pinned_clears_rect() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_active: false,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert_eq!(next.drawing, DrawingState::Armed);
        assert!(next.pinned_rects.is_empty());
    }

    #[test]
    fn spotlight_active_without_pinned_does_not_push_rect() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            spotlight_active: true,
            pinned_active: false,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert!(next.pinned_rects.is_empty(), "spotlight without pinned must not push a rect");
        assert!(!next.spotlight_active, "spotlight_active resets after mouse up");
    }

    // --- Pinned mode: multiple rects accumulate ---

    #[test]
    fn multiple_pinned_rects_accumulate() {
        let mut state = AppState::default();
        // First rect: modifier -> toggle -> draw -> mouse up
        state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        state = process_event(&state, &InputEvent::DigitPressed(1));
        state = process_event(&state, &InputEvent::MouseButtonDown { x: 10, y: 10 });
        state = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 50 });
        assert_eq!(state.pinned_rects.len(), 1);
        assert_eq!(state.pinned_rects[0], PinnedRect { x0: 10, y0: 10, x1: 50, y1: 50, spotlight: false });

        // Second rect: still modifier held, draw another (pinned_active reset, need to toggle again)
        state = process_event(&state, &InputEvent::DigitPressed(1));
        state = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 100 });
        state = process_event(&state, &InputEvent::MouseButtonUp { x: 200, y: 200 });
        assert_eq!(state.pinned_rects.len(), 2);
        assert_eq!(state.pinned_rects[1], PinnedRect { x0: 100, y0: 100, x1: 200, y1: 200, spotlight: false });
    }

    // --- Pinned mode: per-rect reset ---

    #[test]
    fn pinned_active_resets_after_mouse_up() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert!(!next.pinned_active);
    }

    // --- EscapePressed ---

    #[test]
    fn escape_clears_pinned_rects() {
        let state = AppState {
            drawing: DrawingState::Armed,
            pinned_rects: vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: false }, PinnedRect { x0: 100, y0: 100, x1: 200, y1: 200, spotlight: false }],
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::EscapePressed);
        assert!(next.pinned_rects.is_empty());
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn escape_during_draw_cancels_and_clears_pinned() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_rects: vec![PinnedRect { x0: 0, y0: 0, x1: 100, y1: 100, spotlight: false }],
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::EscapePressed);
        assert_eq!(next.drawing, DrawingState::Armed);
        assert!(next.pinned_rects.is_empty());
        assert!(!next.pinned_active);
    }

    #[test]
    fn escape_in_idle_clears_pinned_rects() {
        let state = AppState {
            drawing: DrawingState::Idle,
            pinned_rects: vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: false }],
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::EscapePressed);
        assert!(next.pinned_rects.is_empty());
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    // --- Modifier release resets pinned_active ---

    #[test]
    fn modifier_release_resets_pinned_active() {
        let state = AppState {
            drawing: DrawingState::Armed,
            pinned_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert!(!next.pinned_active);
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn drawing_modifier_release_with_pinned_pushes_rect() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.pinned_rects, vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: false }]);
        assert!(!next.pinned_active);
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    // --- Spotlight mode: DigitPressed(2) toggle ---

    #[test]
    fn armed_digit_2_toggles_spotlight_active() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(2));
        assert!(next.spotlight_active);
        assert_eq!(next.drawing, DrawingState::Armed);
    }

    #[test]
    fn armed_digit_2_toggle_off() {
        let state = AppState { drawing: DrawingState::Armed, spotlight_active: true, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(2));
        assert!(!next.spotlight_active);
    }

    #[test]
    fn drawing_digit_2_toggles_spotlight_active() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::DigitPressed(2));
        assert!(next.spotlight_active);
        assert_eq!(next.drawing, DrawingState::Drawing { start: (10, 20), current: (50, 80) });
    }

    #[test]
    fn idle_digit_2_is_noop() {
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(2));
        assert!(!next.spotlight_active);
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    // --- Spotlight mode: mouse up with spotlight ---

    #[test]
    fn drawing_mouse_up_pinned_spotlight_pushes_rect_with_spotlight_true() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_active: true,
            spotlight_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert_eq!(next.pinned_rects.len(), 1);
        assert!(next.pinned_rects[0].spotlight);
        assert!(!next.spotlight_active, "spotlight_active resets after mouse up");
    }

    #[test]
    fn drawing_mouse_up_pinned_no_spotlight_pushes_spotlight_false() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_active: true,
            spotlight_active: false,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert_eq!(next.pinned_rects.len(), 1);
        assert!(!next.pinned_rects[0].spotlight);
    }

    #[test]
    fn spotlight_active_resets_after_mouse_up() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            spotlight_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 80 });
        assert!(!next.spotlight_active);
    }

    // --- Spotlight + Pinned independence ---

    #[test]
    fn pinned_and_spotlight_independent() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(1));
        let next = process_event(&next, &InputEvent::DigitPressed(2));
        assert!(next.pinned_active);
        assert!(next.spotlight_active);
    }

    #[test]
    fn digit_1_does_not_affect_spotlight() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(1));
        assert!(!next.spotlight_active);
    }

    #[test]
    fn digit_2_does_not_affect_pinned() {
        let state = AppState { drawing: DrawingState::Armed, ..Default::default() };
        let next = process_event(&state, &InputEvent::DigitPressed(2));
        assert!(!next.pinned_active);
    }

    // --- Spotlight: EscapePressed ---

    #[test]
    fn escape_resets_spotlight_active() {
        let state = AppState {
            drawing: DrawingState::Armed,
            spotlight_active: true,
            pinned_rects: vec![PinnedRect { x0: 10, y0: 20, x1: 50, y1: 80, spotlight: true }],
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::EscapePressed);
        assert!(!next.spotlight_active);
        assert!(next.pinned_rects.is_empty());
    }

    // --- Spotlight: modifier release ---

    #[test]
    fn modifier_release_resets_spotlight_active() {
        let state = AppState {
            drawing: DrawingState::Armed,
            spotlight_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert!(!next.spotlight_active);
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    #[test]
    fn drawing_modifier_release_with_pinned_spotlight_pushes_rect() {
        let state = AppState {
            drawing: DrawingState::Drawing { start: (10, 20), current: (50, 80) },
            pinned_active: true,
            spotlight_active: true,
            ..Default::default()
        };
        let next = process_event(&state, &InputEvent::ModifierChanged { pressed: false });
        assert_eq!(next.pinned_rects.len(), 1);
        assert!(next.pinned_rects[0].spotlight);
        assert!(!next.spotlight_active);
        assert_eq!(next.drawing, DrawingState::Idle);
    }

    // --- Spotlight: multiple rects accumulate ---

    #[test]
    fn multiple_spotlight_rects_accumulate() {
        let mut state = AppState::default();
        // First spotlight rect
        state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        state = process_event(&state, &InputEvent::DigitPressed(2));
        state = process_event(&state, &InputEvent::DigitPressed(1)); // also pinned
        state = process_event(&state, &InputEvent::MouseButtonDown { x: 10, y: 10 });
        state = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 50 });
        assert_eq!(state.pinned_rects.len(), 1);
        assert!(state.pinned_rects[0].spotlight);

        // Second spotlight rect (must re-toggle)
        state = process_event(&state, &InputEvent::DigitPressed(2));
        state = process_event(&state, &InputEvent::DigitPressed(1));
        state = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 100 });
        state = process_event(&state, &InputEvent::MouseButtonUp { x: 200, y: 200 });
        assert_eq!(state.pinned_rects.len(), 2);
        assert!(state.pinned_rects[1].spotlight);
    }

    // --- Mixed spotlight and non-spotlight ---

    #[test]
    fn mixed_spotlight_and_non_spotlight_rects() {
        let mut state = AppState::default();
        // Non-spotlight pinned rect
        state = process_event(&state, &InputEvent::ModifierChanged { pressed: true });
        state = process_event(&state, &InputEvent::DigitPressed(1));
        state = process_event(&state, &InputEvent::MouseButtonDown { x: 10, y: 10 });
        state = process_event(&state, &InputEvent::MouseButtonUp { x: 50, y: 50 });
        assert!(!state.pinned_rects[0].spotlight);

        // Spotlight pinned rect
        state = process_event(&state, &InputEvent::DigitPressed(1));
        state = process_event(&state, &InputEvent::DigitPressed(2));
        state = process_event(&state, &InputEvent::MouseButtonDown { x: 100, y: 100 });
        state = process_event(&state, &InputEvent::MouseButtonUp { x: 200, y: 200 });
        assert!(!state.pinned_rects[0].spotlight, "first rect unchanged");
        assert!(state.pinned_rects[1].spotlight, "second rect is spotlight");
    }
}
