use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use super::{PopupManager, PopupContent};

const STATUS_HEIGHT: i32 = 44;
const STATUS_RADIUS: i32 = 12;
const STATUS_PADDING_H: i32 = 24;
const STATUS_TOP_MARGIN: i32 = 48;

const CHEATSHEET_WIDTH: i32 = 320;
const CHEATSHEET_ROW_HEIGHT: i32 = 32;
const CHEATSHEET_PADDING_V: i32 = 20;
const CHEATSHEET_RADIUS: i32 = 14;
const CHEATSHEET_PADDING_H: i32 = 24;

const BG_R: u8 = 28;
const BG_G: u8 = 28;
const BG_B: u8 = 30;
const BG_A_STATUS: u8 = 224;      // ~0.88
const BG_A_CHEATSHEET: u8 = 235;  // ~0.92

const SHADOW_COLOR: (u8, u8, u8) = (0, 0, 0);

pub struct GdiRenderer {
    hwnd: HWND,
    mem_dc: HDC,
    mem_bitmap: HBITMAP,
    original_stock_bitmap: HBITMAP,  // never deleted — restored in Drop
    font_normal: HFONT,
    font_key: HFONT,
    font_desc: HFONT,
    current_width: i32,
    current_height: i32,
    pixels: *mut u8,
}

impl GdiRenderer {
    pub fn new(hwnd: HWND) -> Self {
        unsafe {
            let screen_dc = GetDC(HWND::default()); // screen DC, not window DC
            let mem_dc = CreateCompatibleDC(screen_dc);

            // Initial size — will be resized on first render
            let width = 400;
            let height = 300;

            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut pixels: *mut u8 = std::ptr::null_mut();
            let mem_bitmap = CreateDIBSection(None, &bi, DIB_RGB_COLORS, &mut pixels as *mut *mut u8 as _, None, 0)
                .expect("CreateDIBSection failed");
            let original_stock_bitmap = SelectObject(mem_dc, mem_bitmap);
            let original_stock_bitmap = HBITMAP(original_stock_bitmap.0);

            let font_normal = create_font(14, FW_MEDIUM.0);
            let font_key = create_font(13, FW_SEMIBOLD.0);
            let font_desc = create_font(13, FW_NORMAL.0);

            ReleaseDC(HWND::default(), screen_dc);

            Self {
                hwnd,
                mem_dc,
                mem_bitmap,
                original_stock_bitmap,
                font_normal,
                font_key,
                font_desc,
                current_width: width,
                current_height: height,
                pixels,
            }
        }
    }

    fn ensure_size(&mut self, width: i32, height: i32) {
        if width == self.current_width && height == self.current_height {
            return;
        }
        unsafe {
            // Deselect current bitmap before deleting
            SelectObject(self.mem_dc, self.original_stock_bitmap);
            let _ = DeleteObject(self.mem_bitmap);

            let screen_dc = GetDC(HWND::default());
            let bi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut pixels: *mut u8 = std::ptr::null_mut();
            let mem_bitmap = CreateDIBSection(None, &bi, DIB_RGB_COLORS, &mut pixels as *mut *mut u8 as _, None, 0)
                .expect("CreateDIBSection failed");
            // Select new bitmap — don't update original_stock_bitmap
            SelectObject(self.mem_dc, mem_bitmap);
            self.mem_bitmap = mem_bitmap;
            self.pixels = pixels;
            self.current_width = width;
            self.current_height = height;
            ReleaseDC(HWND::default(), screen_dc);
        }
    }

    pub fn render(&mut self, manager: &PopupManager, monitor_rect: (i32, i32, i32, i32)) {
        if !manager.is_visible() {
            unsafe { let _ = ShowWindow(self.hwnd, SW_HIDE); }
            return;
        }

        let (mon_left, mon_top, mon_right, mon_bottom) = monitor_rect;
        let mon_width = mon_right - mon_left;
        let mon_height = mon_bottom - mon_top;
        let y_offset = manager.current_y_offset() as i32;

        match manager.content() {
            PopupContent::Status => {
                self.render_status(manager, mon_left, mon_top, mon_width, y_offset);
            }
            PopupContent::Cheatsheet => {
                self.render_cheatsheet(manager, mon_left, mon_top, mon_width, mon_height, y_offset);
            }
        }
    }

    fn render_status(&mut self, manager: &PopupManager, mon_left: i32, mon_top: i32, mon_width: i32, y_offset: i32) {
        let text = manager.status_text();
        let text_w = measure_text_width(self.mem_dc, self.font_normal, text);
        let popup_w = text_w + STATUS_PADDING_H * 2;
        let popup_h = STATUS_HEIGHT;

        // Add shadow margin
        let shadow_margin = 8;
        let buf_w = popup_w + shadow_margin * 2;
        let buf_h = popup_h + shadow_margin * 2;

        self.ensure_size(buf_w, buf_h);
        unsafe {
            clear_buffer(self.pixels, buf_w, buf_h);

            let card_x = shadow_margin;
            let card_y = shadow_margin;

            // Shadow layers
            paint_shadow(self.pixels, buf_w, buf_h, card_x + 2, card_y + 2, popup_w, popup_h, STATUS_RADIUS, SHADOW_COLOR, 60);

            // Card background
            paint_rounded_rect(self.pixels, buf_w, buf_h, card_x, card_y, popup_w, popup_h, STATUS_RADIUS, BG_R, BG_G, BG_B, BG_A_STATUS);

            // Text
            SelectObject(self.mem_dc, self.font_normal);
            SetBkMode(self.mem_dc, TRANSPARENT);
            SetTextColor(self.mem_dc, COLORREF(0x00FFFFFF)); // white
            let mut text_wide: Vec<u16> = text.encode_utf16().collect();
            let mut text_rect = RECT {
                left: card_x + STATUS_PADDING_H,
                top: card_y,
                right: card_x + popup_w - STATUS_PADDING_H,
                bottom: card_y + popup_h,
            };
            DrawTextW(self.mem_dc, &mut text_wide, &mut text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

            // Position and show window
            let x = mon_left + (mon_width - buf_w) / 2;
            let y = mon_top + STATUS_TOP_MARGIN + y_offset;

            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(self.hwnd, HWND_TOPMOST, x, y, buf_w, buf_h, SWP_NOACTIVATE);

            commit_layered(self.hwnd, self.mem_dc, buf_w, buf_h);
        }
    }

    fn render_cheatsheet(&mut self, manager: &PopupManager, mon_left: i32, mon_top: i32, mon_width: i32, mon_height: i32, y_offset: i32) {
        let rows = manager.cheatsheet_rows();
        let row_count = rows.len() as i32;
        let popup_w = CHEATSHEET_WIDTH;
        let popup_h = CHEATSHEET_PADDING_V * 2 + row_count * CHEATSHEET_ROW_HEIGHT;

        let shadow_margin = 12;
        let buf_w = popup_w + shadow_margin * 2;
        let buf_h = popup_h + shadow_margin * 2;

        self.ensure_size(buf_w, buf_h);
        unsafe {
            clear_buffer(self.pixels, buf_w, buf_h);

            let card_x = shadow_margin;
            let card_y = shadow_margin;

            // Shadow
            paint_shadow(self.pixels, buf_w, buf_h, card_x + 4, card_y + 4, popup_w, popup_h, CHEATSHEET_RADIUS, SHADOW_COLOR, 80);

            // Card background
            paint_rounded_rect(self.pixels, buf_w, buf_h, card_x, card_y, popup_w, popup_h, CHEATSHEET_RADIUS, BG_R, BG_G, BG_B, BG_A_CHEATSHEET);

            // Text rows
            for (i, (key, desc)) in rows.iter().enumerate() {
                let row_y = card_y + CHEATSHEET_PADDING_V + i as i32 * CHEATSHEET_ROW_HEIGHT;

                // Key (left-aligned, semibold)
                SelectObject(self.mem_dc, self.font_key);
                SetBkMode(self.mem_dc, TRANSPARENT);
                SetTextColor(self.mem_dc, COLORREF(0x00E5E5E5)); // #E5E5E5
                let mut key_w: Vec<u16> = key.encode_utf16().collect();
                let mut key_rect = RECT {
                    left: card_x + CHEATSHEET_PADDING_H,
                    top: row_y,
                    right: card_x + popup_w / 2,
                    bottom: row_y + CHEATSHEET_ROW_HEIGHT,
                };
                DrawTextW(self.mem_dc, &mut key_w, &mut key_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

                // Desc (right-aligned, regular)
                SelectObject(self.mem_dc, self.font_desc);
                SetTextColor(self.mem_dc, COLORREF(0x00AEAEB2)); // #AEAEB2
                let mut desc_w: Vec<u16> = desc.encode_utf16().collect();
                let mut desc_rect = RECT {
                    left: card_x + popup_w / 2,
                    top: row_y,
                    right: card_x + popup_w - CHEATSHEET_PADDING_H,
                    bottom: row_y + CHEATSHEET_ROW_HEIGHT,
                };
                DrawTextW(self.mem_dc, &mut desc_w, &mut desc_rect, DT_RIGHT | DT_VCENTER | DT_SINGLELINE);
            }

            // Position: centered on monitor
            let x = mon_left + (mon_width - buf_w) / 2;
            let y = mon_top + (mon_height - buf_h) / 2 + y_offset;

            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(self.hwnd, HWND_TOPMOST, x, y, buf_w, buf_h, SWP_NOACTIVATE);

            commit_layered(self.hwnd, self.mem_dc, buf_w, buf_h);
        }
    }
}

impl Drop for GdiRenderer {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem_dc, self.original_stock_bitmap);
            let _ = DeleteObject(self.mem_bitmap);
            let _ = DeleteDC(self.mem_dc);
            let _ = DeleteObject(self.font_normal);
            let _ = DeleteObject(self.font_key);
            let _ = DeleteObject(self.font_desc);
        }
    }
}

unsafe fn create_font(size: i32, weight: u32) -> HFONT {
    let face_name: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
    CreateFontW(
        -size, 0, 0, 0, weight as i32, 0, 0, 0,
        DEFAULT_CHARSET.0 as u32, OUT_DEFAULT_PRECIS.0 as u32,
        CLIP_DEFAULT_PRECIS.0 as u32, CLEARTYPE_QUALITY.0 as u32,
        DEFAULT_PITCH.0 as u32,
        windows::core::PCWSTR(face_name.as_ptr()),
    )
}

unsafe fn clear_buffer(pixels: *mut u8, width: i32, height: i32) {
    debug_assert!(width > 0 && height > 0, "clear_buffer: width={width}, height={height}");
    let total = (width * height * 4) as usize;
    std::ptr::write_bytes(pixels, 0, total);
}

unsafe fn paint_rounded_rect(pixels: *mut u8, buf_w: i32, buf_h: i32, x: i32, y: i32, w: i32, h: i32, radius: i32, r: u8, g: u8, b: u8, a: u8) {
    debug_assert!(x >= 0 && y >= 0, "paint_rounded_rect: x={x}, y={y}");
    for py in 0..h {
        for px in 0..w {
            let corner_alpha = rounded_corner_alpha(px, py, w, h, radius);
            if corner_alpha <= 0.0 {
                continue;
            }
            let alpha = (a as f32 * corner_alpha) as u8;
            let idx = (((y + py) * buf_w + (x + px)) * 4) as usize;
            if idx + 3 >= (buf_w * buf_h * 4) as usize {
                continue;
            }
            blend_pixel(pixels.add(idx), r, g, b, alpha);
        }
    }
}

unsafe fn paint_shadow(pixels: *mut u8, buf_w: i32, buf_h: i32, x: i32, y: i32, w: i32, h: i32, radius: i32, color: (u8, u8, u8), base_alpha: u8) {
    // Layered shadow: 3 layers at increasing offsets and decreasing alpha
    for i in 0..3 {
        let offset = (i + 1) as i32;
        let alpha = (base_alpha as u32 * (3 - i) as u32 / 3) as u8;
        paint_rounded_rect(pixels, buf_w, buf_h, x + offset, y + offset, w, h, radius + offset, color.0, color.1, color.2, alpha);
    }
}

fn rounded_corner_alpha(px: i32, py: i32, w: i32, h: i32, radius: i32) -> f32 {
    let r = radius as f32;
    let (cx, cy) = if px < radius && py < radius {
        (r, r) // top-left
    } else if px >= w - radius && py < radius {
        (w as f32 - r - 1.0, r) // top-right
    } else if px < radius && py >= h - radius {
        (r, h as f32 - r - 1.0) // bottom-left
    } else if px >= w - radius && py >= h - radius {
        (w as f32 - r - 1.0, h as f32 - r - 1.0) // bottom-right
    } else {
        return 1.0; // not in a corner
    };
    let dx = px as f32 - cx;
    let dy = py as f32 - cy;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist <= r - 1.0 {
        1.0
    } else if dist >= r + 1.0 {
        0.0
    } else {
        (r + 1.0 - dist) / 2.0 // smooth 2px anti-alias
    }
}

unsafe fn blend_pixel(dst: *mut u8, r: u8, g: u8, b: u8, a: u8) {
    if a == 0 { return; }
    let alpha = a as f32 / 255.0;
    let inv = 1.0 - alpha;
    *dst = (b as f32 * alpha + *dst as f32 * inv) as u8;       // B
    *dst.add(1) = (g as f32 * alpha + *dst.add(1) as f32 * inv) as u8; // G
    *dst.add(2) = (r as f32 * alpha + *dst.add(2) as f32 * inv) as u8; // R
    *dst.add(3) = (a as f32 + *dst.add(3) as f32 * inv).min(255.0) as u8; // A
}

/// Push pixels to the layered window. mem_dc already has mem_bitmap selected.
unsafe fn commit_layered(hwnd: HWND, mem_dc: HDC, width: i32, height: i32) {
    let size = SIZE { cx: width, cy: height };
    let point = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let _ = UpdateLayeredWindow(
        hwnd,
        None,
        Some(&point),
        Some(&size),
        mem_dc,
        Some(&point),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );
}

fn measure_text_width(dc: HDC, font: HFONT, text: &str) -> i32 {
    unsafe {
        let old = SelectObject(dc, font);
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
        SelectObject(dc, old);
        size.cx
    }
}
