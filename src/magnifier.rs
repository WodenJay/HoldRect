use std::f64::consts::PI;

/// Default magnifier diameter in pixels
pub const MAGNIFIER_DIAMETER: i32 = 350;

/// Rainbow border width in pixels
pub(crate) const BORDER_WIDTH: i32 = 4;

/// Compute perimeter position (0.0..1.0) around a circle for a point on the border.
/// Uses atan2 angle, starting from right (3 o'clock), going clockwise.
pub fn circular_perimeter_position(x: i32, y: i32, cx: i32, cy: i32) -> f32 {
    let dx = (x - cx) as f64;
    let dy = (y - cy) as f64;
    let angle = dy.atan2(dx); // -PI..PI
    let normalized = (angle + PI) / (2.0 * PI); // 0..1
    normalized as f32
}

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Magnifier window -- separate WS_POPUP with circular clip, screen capture, zoom.
#[cfg(windows)]
pub struct MagnifierWindow {
    #[allow(dead_code)]
    hwnd: HWND,
    #[allow(dead_code)]
    diameter: i32,
}

#[cfg(windows)]
impl MagnifierWindow {
    pub fn new(diameter: i32, overlay_hwnd: HWND) -> Self {
        let _ = overlay_hwnd;
        // Window creation will be implemented in Task 4
        todo!("MagnifierWindow::new")
    }

    pub fn render(
        &mut self,
        cursor_pos: (i32, i32),
        zoom: f64,
        color_mode: &crate::config::ColorMode,
        time_offset: f32,
    ) {
        let _ = (cursor_pos, zoom, color_mode, time_offset);
        todo!("MagnifierWindow::render")
    }

    pub fn hide(&self) {
        todo!("MagnifierWindow::hide")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_perimeter_right_is_zero() {
        let pos = circular_perimeter_position(100, 50, 50, 50);
        assert!(
            (pos - 0.5).abs() < 0.01,
            "right (0 deg) should map to ~0.5, got {}",
            pos
        );
    }

    #[test]
    fn circular_perimeter_top_is_quarter() {
        let pos = circular_perimeter_position(50, 0, 50, 50);
        assert!(
            (pos - 0.25).abs() < 0.01,
            "top (270 deg / -90 deg) should map to ~0.25, got {}",
            pos
        );
    }

    #[test]
    fn circular_perimeter_wraps_around() {
        let pos1 = circular_perimeter_position(50, 50 + 10, 50, 50); // bottom
        let pos2 = circular_perimeter_position(50, 50 - 10, 50, 50); // top
        assert!(
            (pos1 - pos2).abs() > 0.4,
            "opposite sides should be ~0.5 apart"
        );
    }

    #[test]
    fn circular_perimeter_same_point_is_defined() {
        // When point == center, atan2(0,0) is defined (0.0 in Rust)
        let pos = circular_perimeter_position(50, 50, 50, 50);
        assert!((0.0..=1.0).contains(&pos));
    }
}
