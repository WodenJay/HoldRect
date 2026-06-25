pub mod animation;
#[cfg(windows)]
pub mod gdi_renderer;

use std::time::Instant;
use crate::state::{AppState, InputEvent};

const SLIDE_IN_DURATION_MS: u64 = 350;
const HOLD_DURATION_MS: u64 = 1000;
const SLIDE_OUT_DURATION_MS: u64 = 200;
const START_Y_OFFSET: f64 = -96.0;
const TARGET_Y_OFFSET: f64 = 0.0;

// Spring params: slide-in (slightly underdamped)
const SLIDE_IN_OMEGA: f64 = 20.0;
const SLIDE_IN_ZETA: f64 = 0.78;

// Spring params: slide-out (critically damped)
const SLIDE_OUT_OMEGA: f64 = 26.0;
const SLIDE_OUT_ZETA: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub enum PopupContent {
    Status,
    Cheatsheet,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PopupPhase {
    Hidden,
    SlidingIn { started_at: Instant, from_y: f64 },
    Holding { started_at: Instant },
    SlidingOut { started_at: Instant, from_y: f64 },
}

pub fn build_status_text(pinned: bool, spotlight: bool) -> String {
    match (pinned, spotlight) {
        (false, false) => "Transient".to_string(),
        (true, false) => "Pinned".to_string(),
        (false, true) => "Spotlight".to_string(),
        (true, true) => "Pinned \u{00b7} Spotlight".to_string(),
    }
}

pub struct PopupManager {
    content: PopupContent,
    phase: PopupPhase,
    status_text: String,
    cheatsheet_rows: Vec<(String, String)>,
}

impl PopupManager {
    pub fn new(modifier_name: &str) -> Self {
        let drag_label = format!("{} + drag", modifier_name);
        let help_label = format!("{} + `", modifier_name);
        Self {
            content: PopupContent::Status,
            phase: PopupPhase::Hidden,
            status_text: String::new(),
            cheatsheet_rows: vec![
                (drag_label, "Draw".to_string()),
                ("1".to_string(), "Pin".to_string()),
                ("2".to_string(), "Spotlight".to_string()),
                ("3".to_string(), "Magnifier".to_string()),
                ("Esc".to_string(), "Clear".to_string()),
                (help_label, "Help".to_string()),
            ],
        }
    }

    pub fn on_event(&mut self, event: &InputEvent, state: &AppState) {
        match event {
            InputEvent::DigitPressed(3) => {
                // Cheatsheet suppresses status popup (same rule as digit 1/2)
                if self.content == PopupContent::Cheatsheet && self.phase != PopupPhase::Hidden {
                    return;
                }
                if state.magnifier_active
                    && matches!(state.drawing, crate::state::DrawingState::Armed | crate::state::DrawingState::Drawing { .. })
                {
                    self.show_status("Magnifier");
                }
            }
            InputEvent::DigitPressed(1) | InputEvent::DigitPressed(2) => {
                // Cheatsheet suppresses status popup (spec: mutually exclusive)
                if self.content == PopupContent::Cheatsheet && self.phase != PopupPhase::Hidden {
                    return;
                }
                if matches!(state.drawing, crate::state::DrawingState::Armed | crate::state::DrawingState::Drawing { .. }) {
                    let text = build_status_text(state.pinned_active, state.spotlight_active);
                    self.show_status(&text);
                }
            }
            InputEvent::ToggleHelp => {
                self.show_cheatsheet();
            }
            InputEvent::HideHelp => {
                self.hide_cheatsheet();
            }
            _ => {}
        }
    }

    pub fn show_status(&mut self, text: &str) {
        self.status_text = text.to_string();
        match &self.phase {
            PopupPhase::Hidden => {
                self.content = PopupContent::Status;
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now(), from_y: START_Y_OFFSET };
            }
            PopupPhase::SlidingIn { .. } => {
                // Keep sliding-in animation, just update text — don't snap to Holding
                self.content = PopupContent::Status;
            }
            PopupPhase::Holding { .. } => {
                self.content = PopupContent::Status;
                self.phase = PopupPhase::Holding { started_at: Instant::now() };
            }
            PopupPhase::SlidingOut { .. } => {
                // Reverse: restart slide-in from current position
                self.content = PopupContent::Status;
                let from_y = self.current_y_offset();
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now(), from_y };
            }
        }
    }

    pub fn show_cheatsheet(&mut self) {
        match &self.phase {
            PopupPhase::Hidden => {
                self.content = PopupContent::Cheatsheet;
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now(), from_y: START_Y_OFFSET };
            }
            PopupPhase::SlidingIn { .. } | PopupPhase::Holding { .. } => {
                // Already showing — no-op for cheatsheet
                if self.content == PopupContent::Cheatsheet {
                    return;
                }
                // Status popup active — replace with cheatsheet
                self.content = PopupContent::Cheatsheet;
                self.phase = PopupPhase::Holding { started_at: Instant::now() };
            }
            PopupPhase::SlidingOut { .. } => {
                self.content = PopupContent::Cheatsheet;
                let from_y = self.current_y_offset();
                self.phase = PopupPhase::SlidingIn { started_at: Instant::now(), from_y };
            }
        }
    }

    pub fn hide_cheatsheet(&mut self) {
        if self.content != PopupContent::Cheatsheet {
            return;
        }
        match &self.phase {
            PopupPhase::SlidingIn { .. } | PopupPhase::Holding { .. } => {
                let from_y = self.current_y_offset();
                self.phase = PopupPhase::SlidingOut { started_at: Instant::now(), from_y };
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let new_phase = match &self.phase {
            PopupPhase::SlidingIn { started_at, .. } => {
                let elapsed = now.duration_since(*started_at).as_millis() as u64;
                if elapsed >= SLIDE_IN_DURATION_MS {
                    Some(PopupPhase::Holding { started_at: now })
                } else {
                    None
                }
            }
            PopupPhase::Holding { started_at } => {
                let elapsed = now.duration_since(*started_at).as_millis() as u64;
                if self.content == PopupContent::Cheatsheet {
                    None // cheatsheet has no hold timer
                } else if elapsed >= HOLD_DURATION_MS {
                    let from_y = self.current_y_offset();
                    Some(PopupPhase::SlidingOut { started_at: now, from_y })
                } else {
                    None
                }
            }
            PopupPhase::SlidingOut { started_at, .. } => {
                let elapsed = now.duration_since(*started_at).as_millis() as u64;
                if elapsed >= SLIDE_OUT_DURATION_MS {
                    Some(PopupPhase::Hidden)
                } else {
                    None
                }
            }
            PopupPhase::Hidden => None,
        };
        if let Some(phase) = new_phase {
            self.phase = phase;
        }
    }

    pub fn needs_frame(&self) -> bool {
        !matches!(self.phase, PopupPhase::Hidden)
    }

    pub fn is_visible(&self) -> bool {
        self.needs_frame()
    }

    pub fn content(&self) -> &PopupContent {
        &self.content
    }

    pub fn status_text(&self) -> &str {
        &self.status_text
    }

    pub fn cheatsheet_rows(&self) -> &[(String, String)] {
        &self.cheatsheet_rows
    }

    pub fn update_modifier_name(&mut self, name: &str) {
        let drag_label = format!("{} + drag", name);
        let help_label = format!("{} + `", name);
        self.cheatsheet_rows = vec![
            (drag_label, "Draw".to_string()),
            ("1".to_string(), "Pin".to_string()),
            ("2".to_string(), "Spotlight".to_string()),
            ("3".to_string(), "Magnifier".to_string()),
            ("Esc".to_string(), "Clear".to_string()),
            (help_label, "Help".to_string()),
        ];
    }

    pub fn current_y_offset(&self) -> f64 {
        match &self.phase {
            PopupPhase::Hidden => START_Y_OFFSET,
            PopupPhase::SlidingIn { started_at, from_y } => {
                let t = started_at.elapsed().as_secs_f64();
                animation::spring_position(t, *from_y, TARGET_Y_OFFSET, SLIDE_IN_OMEGA, SLIDE_IN_ZETA)
            }
            PopupPhase::Holding { .. } => TARGET_Y_OFFSET,
            PopupPhase::SlidingOut { started_at, from_y } => {
                let t = started_at.elapsed().as_secs_f64();
                animation::spring_position(t, *from_y, START_Y_OFFSET, SLIDE_OUT_OMEGA, SLIDE_OUT_ZETA)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DrawingState;

    fn make_manager() -> PopupManager {
        PopupManager::new("Alt")
    }

    fn armed_state() -> AppState {
        AppState { drawing: DrawingState::Armed, ..Default::default() }
    }

    // --- build_status_text ---

    #[test]
    fn build_status_text_transient() {
        assert_eq!(build_status_text(false, false), "Transient");
    }

    #[test]
    fn build_status_text_pinned() {
        assert_eq!(build_status_text(true, false), "Pinned");
    }

    #[test]
    fn build_status_text_spotlight() {
        assert_eq!(build_status_text(false, true), "Spotlight");
    }

    #[test]
    fn build_status_text_both() {
        assert_eq!(build_status_text(true, true), "Pinned \u{00b7} Spotlight");
    }

    // --- show_status from Hidden ---

    #[test]
    fn show_status_from_hidden_enters_sliding_in() {
        let mut m = make_manager();
        m.show_status("Pinned");
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
        assert_eq!(m.content, PopupContent::Status);
        assert_eq!(m.status_text, "Pinned");
    }

    // --- show_status from SlidingIn ---

    #[test]
    fn show_status_from_sliding_in_keeps_sliding_in() {
        let mut m = make_manager();
        m.show_status("Pinned");
        std::thread::sleep(std::time::Duration::from_millis(50));
        m.show_status("Pinned \u{00b7} Spotlight");
        assert_eq!(m.status_text, "Pinned \u{00b7} Spotlight");
        // Should stay SlidingIn — no snap to Holding
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
    }

    // --- show_status from SlidingOut ---

    #[test]
    fn show_status_from_sliding_out_reverses_to_sliding_in() {
        let mut m = make_manager();
        m.show_status("Pinned");
        // Fast-forward to Holding
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() - std::time::Duration::from_millis(2000) };
        m.tick(); // -> SlidingOut
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
        m.show_status("Spotlight");
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
    }

    // --- tick transitions ---

    #[test]
    fn tick_sliding_in_to_holding_after_duration() {
        let mut m = make_manager();
        m.show_status("Pinned");
        m.phase = PopupPhase::SlidingIn { started_at: std::time::Instant::now() - std::time::Duration::from_millis(500), from_y: START_Y_OFFSET };
        m.tick();
        assert!(matches!(m.phase, PopupPhase::Holding { .. }));
    }

    #[test]
    fn tick_holding_to_sliding_out_after_duration() {
        let mut m = make_manager();
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() - std::time::Duration::from_millis(1100) };
        m.tick();
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
    }

    #[test]
    fn tick_sliding_out_to_hidden_after_duration() {
        let mut m = make_manager();
        m.phase = PopupPhase::SlidingOut { started_at: std::time::Instant::now() - std::time::Duration::from_millis(400), from_y: 0.0 };
        m.tick();
        assert_eq!(m.phase, PopupPhase::Hidden);
    }

    // --- cheatsheet ---

    #[test]
    fn show_cheatsheet_from_hidden() {
        let mut m = make_manager();
        m.show_cheatsheet();
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
        assert_eq!(m.content, PopupContent::Cheatsheet);
    }

    #[test]
    fn show_cheatsheet_already_showing_is_noop() {
        let mut m = make_manager();
        m.show_cheatsheet();
        let phase_before = m.phase.clone();
        m.show_cheatsheet();
        assert_eq!(m.phase, phase_before);
    }

    #[test]
    fn hide_cheatsheet_triggers_sliding_out() {
        let mut m = make_manager();
        m.show_cheatsheet();
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() };
        m.hide_cheatsheet();
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
    }

    #[test]
    fn hide_cheatsheet_ignores_status_popup() {
        let mut m = make_manager();
        m.show_status("Pinned");
        let phase_before = m.phase.clone();
        m.hide_cheatsheet();
        assert_eq!(m.phase, phase_before);
    }

    #[test]
    fn cheatsheet_no_hold_timer() {
        let mut m = make_manager();
        m.show_cheatsheet();
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() - std::time::Duration::from_millis(5000) };
        m.tick();
        // Should still be Holding (no auto-dismiss for cheatsheet)
        assert!(matches!(m.phase, PopupPhase::Holding { .. }));
    }

    // --- on_event integration ---

    #[test]
    fn on_digit_pressed_shows_status() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::DigitPressed(1), &state);
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
    }

    #[test]
    fn on_toggle_help_shows_cheatsheet() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::ToggleHelp, &state);
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
        assert_eq!(m.content, PopupContent::Cheatsheet);
    }

    // --- needs_frame ---

    #[test]
    fn needs_frame_false_when_hidden() {
        let m = make_manager();
        assert!(!m.needs_frame());
    }

    #[test]
    fn needs_frame_true_when_sliding_in() {
        let mut m = make_manager();
        m.show_status("Pinned");
        assert!(m.needs_frame());
    }

    // --- cheatsheet_rows ---

    #[test]
    fn cheatsheet_rows_built_from_modifier() {
        let m = PopupManager::new("Ctrl");
        assert_eq!(m.cheatsheet_rows[0].0, "Ctrl + drag");
        assert_eq!(m.cheatsheet_rows[5].0, "Ctrl + `");
    }

    // --- cheatsheet suppresses status ---

    #[test]
    fn cheatsheet_suppresses_status_popup() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::ToggleHelp, &state);
        assert_eq!(m.content, PopupContent::Cheatsheet);
        // DigitPressed should NOT replace cheatsheet with status
        m.on_event(&InputEvent::DigitPressed(1), &state);
        assert_eq!(m.content, PopupContent::Cheatsheet);
    }

    // --- update_modifier_name ---

    #[test]
    fn update_modifier_name_rebuilds_cheatsheet_rows() {
        let mut m = PopupManager::new("Alt");
        assert_eq!(m.cheatsheet_rows()[0].0, "Alt + drag");
        assert_eq!(m.cheatsheet_rows()[5].0, "Alt + `");
        m.update_modifier_name("Ctrl");
        assert_eq!(m.cheatsheet_rows()[0].0, "Ctrl + drag");
        assert_eq!(m.cheatsheet_rows()[1].0, "1");
        assert_eq!(m.cheatsheet_rows()[2].0, "2");
        assert_eq!(m.cheatsheet_rows()[3].0, "3");
        assert_eq!(m.cheatsheet_rows()[4].0, "Esc");
        assert_eq!(m.cheatsheet_rows()[5].0, "Ctrl + `");
    }

    // --- on_event with HideHelp ---

    #[test]
    fn on_hide_help_hides_cheatsheet() {
        let mut m = make_manager();
        let state = armed_state();
        m.on_event(&InputEvent::ToggleHelp, &state);
        assert!(m.needs_frame());
        m.phase = PopupPhase::Holding { started_at: std::time::Instant::now() };
        m.on_event(&InputEvent::HideHelp, &state);
        assert!(matches!(m.phase, PopupPhase::SlidingOut { .. }));
    }

    // --- on_event digit while idle is noop ---

    #[test]
    fn on_digit_pressed_idle_is_noop() {
        let mut m = make_manager();
        let state = AppState { drawing: DrawingState::Idle, ..Default::default() };
        m.on_event(&InputEvent::DigitPressed(1), &state);
        assert_eq!(m.phase, PopupPhase::Hidden);
    }

    // --- magnifier popup ---

    #[test]
    fn on_digit_3_magnifier_active_shows_status() {
        let mut m = make_manager();
        let state = AppState {
            drawing: DrawingState::Armed,
            magnifier_active: true,
            ..Default::default()
        };
        m.on_event(&InputEvent::DigitPressed(3), &state);
        assert!(matches!(m.phase, PopupPhase::SlidingIn { .. }));
        assert_eq!(m.status_text, "Magnifier");
    }

    #[test]
    fn on_digit_3_magnifier_inactive_no_popup() {
        let mut m = make_manager();
        let state = AppState {
            drawing: DrawingState::Armed,
            magnifier_active: false,
            ..Default::default()
        };
        m.on_event(&InputEvent::DigitPressed(3), &state);
        assert_eq!(m.phase, PopupPhase::Hidden);
    }

    #[test]
    fn cheatsheet_includes_magnifier_row() {
        let m = make_manager();
        let rows = m.cheatsheet_rows();
        assert!(rows.iter().any(|(k, v)| k == "3" && v == "Magnifier"));
    }

    #[test]
    fn on_digit_3_while_idle_is_noop() {
        let mut m = make_manager();
        let state = AppState {
            drawing: DrawingState::Idle,
            magnifier_active: true,
            ..Default::default()
        };
        m.on_event(&InputEvent::DigitPressed(3), &state);
        assert_eq!(m.phase, PopupPhase::Hidden);
    }

    #[test]
    fn on_digit_3_suppressed_by_cheatsheet() {
        let mut m = make_manager();
        let state = AppState {
            drawing: DrawingState::Armed,
            magnifier_active: true,
            ..Default::default()
        };
        m.on_event(&InputEvent::ToggleHelp, &state);
        assert_eq!(m.content, PopupContent::Cheatsheet);
        m.on_event(&InputEvent::DigitPressed(3), &state);
        assert_eq!(m.content, PopupContent::Cheatsheet);
    }
}
