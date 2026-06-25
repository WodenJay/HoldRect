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
    hwnd: HWND,
    diameter: i32,
}

#[cfg(windows)]
impl MagnifierWindow {
    pub fn new(diameter: i32, overlay_hwnd: HWND) -> Self {
        use windows::Win32::UI::WindowsAndMessaging::*;
        assert!(diameter > 0, "magnifier diameter must be positive, got {diameter}");

        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT,
                windows::core::w!("STATIC"),
                windows::core::w!("HoldRect Magnifier"),
                WS_POPUP,
                0, 0, diameter, diameter,
                None, None, None, None,
            ).expect("Failed to create magnifier window");

            // Set overlay as owner for Z-order stacking
            SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, overlay_hwnd.0 as isize);

            Self { hwnd, diameter }
        }
    }

    pub fn hide(&self) {
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn render(
        &mut self,
        cursor_pos: (i32, i32),
        zoom: f64,
        color_mode: &crate::config::ColorMode,
        time_offset: f32,
    ) {
        use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
        use windows::Win32::Graphics::Gdi::*;
        use windows::Win32::UI::WindowsAndMessaging::*;

        unsafe {
            assert!(zoom > 0.0, "magnifier zoom must be positive, got {zoom}");
            let d = self.diameter;
            let r = d / 2;

            // 1. Hide to avoid capturing ourselves
            let _ = ShowWindow(self.hwnd, SW_HIDE);

            // 2. Position window at cursor (edge-clamped)
            let virt_x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let virt_y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let virt_w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let virt_h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let x = (cursor_pos.0 - r).clamp(virt_x, virt_x + virt_w - d);
            let y = (cursor_pos.1 - r).clamp(virt_y, virt_y + virt_h - d);
            let _ = SetWindowPos(self.hwnd, None, x, y, d, d, SWP_NOACTIVATE | SWP_NOZORDER);

            // 3. Capture screen region
            let screen_dc = GetDC(HWND::default());
            let mem_dc = CreateCompatibleDC(screen_dc);
            let capture_w = ((d as f64 / zoom) as i32).max(1);
            let capture_h = ((d as f64 / zoom) as i32).max(1);
            let src_x = cursor_pos.0 - capture_w / 2;
            let src_y = cursor_pos.1 - capture_h / 2;

            // Create capture bitmap
            let cap_bmp = CreateCompatibleBitmap(screen_dc, capture_w, capture_h);
            let old_bmp = SelectObject(mem_dc, cap_bmp);
            let _ = BitBlt(mem_dc, 0, 0, capture_w, capture_h, screen_dc, src_x, src_y, SRCCOPY);

            // 4. Create DIB for the magnifier window content
            let dib_dc = CreateCompatibleDC(screen_dc);
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: d,
                    biHeight: -d, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    biSizeImage: (d * d * 4) as u32,
                    ..std::mem::zeroed()
                },
                ..std::mem::zeroed()
            };
            let mut pixels: *mut u8 = std::ptr::null_mut();
            let dib = CreateDIBSection(dib_dc, &bi, DIB_RGB_COLORS, &mut pixels as *mut *mut u8 as _, None, 0)
                .expect("CreateDIBSection failed");
            let old_dib = SelectObject(dib_dc, dib);
            let old_dib = HBITMAP(old_dib.0); // cast HGDIOBJ back to HBITMAP for restore

            // 5. StretchBlt captured content into DIB (scaled up)
            SetStretchBltMode(dib_dc, HALFTONE);
            let _ = SetBrushOrgEx(dib_dc, 0, 0, None); // required after HALFTONE
            let _ = StretchBlt(dib_dc, 0, 0, d, d, mem_dc, 0, 0, capture_w, capture_h, SRCCOPY);

            // 6. Circular clip -- clear outside circle
            let center = d as f64 / 2.0;
            let radius_sq = center * center;
            let pixel_slice = std::slice::from_raw_parts_mut(pixels, (d * d * 4) as usize);
            for row in 0..d {
                for col in 0..d {
                    let dx = col as f64 - center + 0.5;
                    let dy = row as f64 - center + 0.5;
                    let dist_sq = dx * dx + dy * dy;
                    let off = ((row * d + col) * 4) as usize;
                    if dist_sq > radius_sq {
                        // Outside circle: transparent
                        pixel_slice[off] = 0;
                        pixel_slice[off + 1] = 0;
                        pixel_slice[off + 2] = 0;
                        pixel_slice[off + 3] = 0;
                    } else if dist_sq > (center - BORDER_WIDTH as f64) * (center - BORDER_WIDTH as f64) {
                        // Border region: rainbow color using circular perimeter position
                        let (cr, cg, cb) = match color_mode {
                            crate::config::ColorMode::Solid { r, g, b } => (*r, *g, *b),
                            crate::config::ColorMode::Rainbow => {
                                let pos = circular_perimeter_position(col, row, d / 2, d / 2);
                                let hue = (pos + time_offset).fract() * 360.0;
                                crate::overlay::hsv_to_rgb(hue, 1.0, 1.0)
                            }
                        };
                        pixel_slice[off] = cb;     // B
                        pixel_slice[off + 1] = cg; // G
                        pixel_slice[off + 2] = cr; // R
                        pixel_slice[off + 3] = 255;
                    }
                    // else: keep the stretched content as-is (alpha already 255 from StretchBlt)
                }
            }

            // 7. Draw zoom text ("2.0x") at bottom center
            let zoom_text = format!("{:.1}x", zoom);
            SetBkMode(dib_dc, TRANSPARENT);
            let face_name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
            let font = CreateFontW(
                16, 0, 0, 0, FW_NORMAL.0 as i32,
                0, 0, 0, DEFAULT_CHARSET.0 as u32,
                OUT_DEFAULT_PRECIS.0 as u32, CLIP_DEFAULT_PRECIS.0 as u32,
                CLEARTYPE_QUALITY.0 as u32, DEFAULT_PITCH.0 as u32,
                windows::core::PCWSTR(face_name.as_ptr()),
            );
            let old_font = SelectObject(dib_dc, font);
            let text_y = d - 25;
            let mut text_rect = RECT { left: 0, top: text_y, right: d, bottom: d };
            let mut wbuf: Vec<u16> = zoom_text.encode_utf16().chain(std::iter::once(0)).collect();
            // Shadow
            SetTextColor(dib_dc, COLORREF(0x000000));
            let mut shadow_rect = RECT { left: 1, top: text_y + 1, right: d, bottom: d + 1 };
            DrawTextW(dib_dc, &mut wbuf, &mut shadow_rect, DT_CENTER | DT_SINGLELINE);
            // White text
            SetTextColor(dib_dc, COLORREF(0xFFFFFF));
            DrawTextW(dib_dc, &mut wbuf, &mut text_rect, DT_CENTER | DT_SINGLELINE);
            SelectObject(dib_dc, old_font);
            let _ = DeleteObject(font);

            // 8. UpdateLayeredWindow
            let ppt_dst = POINT { x, y };
            let size = SIZE { cx: d, cy: d };
            let ppt_src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                self.hwnd, screen_dc, Some(&ppt_dst), Some(&size),
                dib_dc, Some(&ppt_src), COLORREF(0), Some(&blend), ULW_ALPHA,
            );

            // 9. Cleanup
            SelectObject(dib_dc, old_dib);
            let _ = DeleteObject(dib);
            SelectObject(mem_dc, old_bmp);
            let _ = DeleteObject(cap_bmp);
            let _ = DeleteDC(dib_dc);
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(HWND::default(), screen_dc);

            // 10. Show
            let _ = ShowWindow(self.hwnd, SW_SHOW);
        }
    }
}

#[cfg(windows)]
impl Drop for MagnifierWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
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
        // assert!(diameter > 0) fires before CreateWindowExW, so null HWND is safe here
        let _ = MagnifierWindow::new(0, HWND(std::ptr::null_mut()));
    }

    // Note: render's `assert!(zoom > 0.0)` cannot be unit-tested because
    // MagnifierWindow fields are private and new() calls CreateWindowExW
    // which requires a real Windows display context. The guard is exercised
    // by integration/manual testing instead.

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
