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

/// Magnifier window — host window with circular region + child magnification control.
///
/// Uses Windows Magnification API (`MagSetWindowSource`) which automatically
/// excludes the magnifier from its own capture source, avoiding recursive zoom.
#[cfg(windows)]
pub struct MagnifierWindow {
    host_hwnd: HWND,
    mag_hwnd: HWND,
    diameter: i32,
}

#[cfg(windows)]
impl MagnifierWindow {
    pub fn new(diameter: i32, _overlay_hwnd: HWND) -> Self {
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::UI::Magnification::*;
        use windows::Win32::UI::WindowsAndMessaging::*;
        assert!(diameter > 0, "magnifier diameter must be positive, got {diameter}");

        unsafe {
            MagInitialize().expect("MagInitialize failed");

            // Host window: layered for border rendering, circular via region
            let host_hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW
                    | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
                windows::core::w!("STATIC"),
                windows::core::w!("HoldRect Magnifier Host"),
                WS_POPUP,
                0, 0, diameter, diameter,
                None, None, None, None,
            ).expect("Failed to create magnifier host");

            // Circular clip region on the host window
            let region = CreateEllipticRgn(0, 0, diameter, diameter);
            let _ = SetWindowRgn(host_hwnd, region, true);

            // Child magnification control (inside border)
            let content_d = diameter - BORDER_WIDTH * 2;
            let mag_hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                WC_MAGNIFIER,
                windows::core::PCWSTR(std::ptr::null()),
                WS_CHILD | WS_VISIBLE,
                BORDER_WIDTH, BORDER_WIDTH,
                content_d.max(1), content_d.max(1),
                host_hwnd, None, None, None,
            ).expect("Failed to create magnifier control");

            // Clip magnifier control to inner circle (avoids covering border ring)
            let mag_region = CreateEllipticRgn(0, 0, content_d, content_d);
            SetWindowRgn(mag_hwnd, mag_region, true);

            // Paint initial border onto the host's layered surface
            Self::paint_border(host_hwnd, diameter, &crate::config::ColorMode::Solid { r: 0, g: 255, b: 0 }, 0.0);

            Self { host_hwnd, mag_hwnd, diameter }
        }
    }

    fn paint_border(
        hwnd: HWND,
        d: i32,
        color_mode: &crate::config::ColorMode,
        time_offset: f32,
    ) {
        use windows::Win32::Foundation::{COLORREF, POINT, SIZE};
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            let screen_dc = GetDC(HWND::default());
            let mem_dc = CreateCompatibleDC(screen_dc);

            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: d,
                    biHeight: -d,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    biSizeImage: (d * d * 4) as u32,
                    ..std::mem::zeroed()
                },
                ..std::mem::zeroed()
            };
            let mut pixels: *mut u8 = std::ptr::null_mut();
            let dib = CreateDIBSection(mem_dc, &bi, DIB_RGB_COLORS, &mut pixels as *mut *mut u8 as _, None, 0)
                .expect("CreateDIBSection failed");
            let old_bmp = SelectObject(mem_dc, dib);
            let old_bmp = windows::Win32::Graphics::Gdi::HBITMAP(old_bmp.0);

            // Fill transparent, then paint border ring
            let pixel_slice = std::slice::from_raw_parts_mut(pixels, (d * d * 4) as usize);
            for b in pixel_slice.iter_mut() { *b = 0; }

            let center = d as f64 / 2.0;
            let outer_sq = center * center;
            let inner_sq = (center - BORDER_WIDTH as f64) * (center - BORDER_WIDTH as f64);
            for row in 0..d {
                for col in 0..d {
                    let dx = col as f64 - center + 0.5;
                    let dy = row as f64 - center + 0.5;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq <= outer_sq && dist_sq > inner_sq {
                        let off = ((row * d + col) * 4) as usize;
                        let (r, g, b) = match color_mode {
                            crate::config::ColorMode::Solid { r, g, b } => (*r, *g, *b),
                            crate::config::ColorMode::Rainbow => {
                                let pos = circular_perimeter_position(col, row, d / 2, d / 2);
                                let hue = (pos + time_offset).fract() * 360.0;
                                crate::overlay::hsv_to_rgb(hue, 1.0, 1.0)
                            }
                        };
                        pixel_slice[off] = b;
                        pixel_slice[off + 1] = g;
                        pixel_slice[off + 2] = r;
                        pixel_slice[off + 3] = 255;
                    }
                }
            }

            let size = SIZE { cx: d, cy: d };
            let ppt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd, screen_dc, None, Some(&size),
                mem_dc, Some(&ppt_src), COLORREF(0), Some(&blend), ULW_ALPHA,
            );

            SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(dib);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(HWND::default(), screen_dc);
        }
    }

    pub fn hide(&self) {
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let _ = ShowWindow(self.host_hwnd, SW_HIDE);
        }
    }

    /// Update magnifier: position host window, repaint border, set source rect + zoom.
    pub fn render(
        &mut self,
        cursor_pos: (i32, i32),
        zoom: f64,
        color_mode: &crate::config::ColorMode,
        time_offset: f32,
    ) {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Gdi::InvalidateRect;
        use windows::Win32::UI::Magnification::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            assert!(zoom > 0.0, "magnifier zoom must be positive, got {zoom}");
            let d = self.diameter;
            let r = d / 2;
            let content_d = d - BORDER_WIDTH * 2;

            // Source rect in desktop coordinates, centered on cursor
            let source_w = ((content_d as f64) / zoom).round() as i32;
            let source_h = ((content_d as f64) / zoom).round() as i32;
            let source_rect = RECT {
                left: cursor_pos.0 - source_w / 2,
                top: cursor_pos.1 - source_h / 2,
                right: cursor_pos.0 - source_w / 2 + source_w,
                bottom: cursor_pos.1 - source_h / 2 + source_h,
            };

            // Zoom transform (MAGTRANSFORM is flat [f32; 9], row-major 3x3)
            let mut transform = MAGTRANSFORM {
                v: [
                    zoom as f32, 0.0, 0.0,
                    0.0, zoom as f32, 0.0,
                    0.0, 0.0, 1.0,
                ],
            };
            let _ = MagSetWindowTransform(self.mag_hwnd, &mut transform);
            let _ = MagSetWindowSource(self.mag_hwnd, source_rect);

            // Position host window centered on cursor
            let _ = SetWindowPos(
                self.host_hwnd, HWND_TOPMOST,
                cursor_pos.0 - r, cursor_pos.1 - r, d, d,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );

            // Repaint border (UpdateLayeredWindow persists content, but we
            // repaint each frame to support animated rainbow and ensure the
            // border is never lost after window repositioning).
            Self::paint_border(self.host_hwnd, d, color_mode, time_offset);

            // Trigger magnifier repaint
            let _ = InvalidateRect(self.mag_hwnd, None, false);
        }
    }
}

#[cfg(windows)]
impl Drop for MagnifierWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(self.host_hwnd);
            let _ = windows::Win32::UI::Magnification::MagUninitialize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "diameter")]
    #[cfg(windows)]
    fn magnifier_window_new_zero_diameter_panics() {
        let _ = MagnifierWindow::new(0, HWND(std::ptr::null_mut()));
    }

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
        let pos1 = circular_perimeter_position(50, 50 + 10, 50, 50);
        let pos2 = circular_perimeter_position(50, 50 - 10, 50, 50);
        assert!(
            (pos1 - pos2).abs() > 0.4,
            "opposite sides should be ~0.5 apart"
        );
    }

    #[test]
    fn circular_perimeter_same_point_is_defined() {
        let pos = circular_perimeter_position(50, 50, 50, 50);
        assert!((0.0..=1.0).contains(&pos));
    }
}
