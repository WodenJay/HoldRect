// Transparent overlay window + GDI rendering

use std::sync::mpsc::Receiver;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowId};

#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;

use crate::config::ColorMode;
use crate::state::AppState;
use crate::state::DrawingState;
use crate::state::InputEvent;
use crate::state::process_event;

const FLOW_SPEED: f32 = 0.1;

/// Cached DIB section for the overlay rendering. Allocated once and reused
/// across frames; only recreated when the window dimensions change.
#[cfg(windows)]
struct DibCache {
    bitmap: windows::Win32::Graphics::Gdi::HBITMAP,
    memory_dc: windows::Win32::Graphics::Gdi::HDC,
    pixels: *mut u8,
    width: i32,
    height: i32,
}

#[cfg(windows)]
impl DibCache {
    fn new(width: i32, height: i32) -> Option<Self> {
        use windows::Win32::Graphics::Gdi::*;

        if width <= 0 || height <= 0 {
            return None;
        }

        let bitmap_info = BITMAPINFO {
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

        let bitmap = unsafe {
            match CreateDIBSection(
                None,
                &bitmap_info,
                DIB_RGB_COLORS,
                &mut pixels as *mut *mut u8 as _,
                None,
                0,
            ) {
                Ok(bitmap) => bitmap,
                Err(_) => return None,
            }
        };

        if pixels.is_null() {
            unsafe { let _ = DeleteObject(bitmap); }
            return None;
        }

        let screen_dc = unsafe { GetDC(windows::Win32::Foundation::HWND::default()) };
        let memory_dc = unsafe { CreateCompatibleDC(screen_dc) };
        unsafe {
            SelectObject(memory_dc, bitmap);
            let _ = ReleaseDC(windows::Win32::Foundation::HWND::default(), screen_dc);
        }

        Some(Self { bitmap, memory_dc, pixels, width, height })
    }

    fn ensure_size(&mut self, width: i32, height: i32) {
        if self.width == width && self.height == height {
            return;
        }
        // Size changed -- drop old and allocate new
        self.destroy();
        if let Some(new_cache) = Self::new(width, height) {
            *self = new_cache;
        }
    }

    fn destroy(&mut self) {
        use windows::Win32::Graphics::Gdi::*;
        unsafe {
            SelectObject(self.memory_dc, windows::Win32::Graphics::Gdi::HBITMAP::default());
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.memory_dc);
        }
        self.pixels = std::ptr::null_mut();
        self.width = 0;
        self.height = 0;
    }
}

#[cfg(windows)]
impl Drop for DibCache {
    fn drop(&mut self) {
        self.destroy();
    }
}

pub struct App {
    window: Option<Window>,
    state: AppState,
    input_rx: Receiver<InputEvent>,
    border_width: i32,
    color_mode: ColorMode,
    #[cfg(windows)]
    dib_cache: Option<DibCache>,
}

impl App {
    pub fn new(input_rx: Receiver<InputEvent>, border_width: i32, color_mode: ColorMode) -> Self {
        Self {
            window: None,
            state: AppState::default(),
            input_rx,
            border_width,
            color_mode,
            #[cfg(windows)]
            dib_cache: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // Cover entire virtual desktop (all monitors)
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        let left   = monitors.iter().map(|m| m.position().x).min().unwrap_or(0);
        let top    = monitors.iter().map(|m| m.position().y).min().unwrap_or(0);
        let right  = monitors.iter().map(|m| m.position().x + m.size().width as i32).max().unwrap_or(1920);
        let bottom = monitors.iter().map(|m| m.position().y + m.size().height as i32).max().unwrap_or(1080);

        let attrs = Window::default_attributes()
            .with_title("HoldRect")
            .with_decorations(false)
            .with_visible(false) // start hidden
            .with_skip_taskbar(true)
            .with_position(winit::dpi::PhysicalPosition::new(left, top))
            .with_inner_size(winit::dpi::PhysicalSize::new(
                (right - left) as u32,
                (bottom - top) as u32,
            ));
        let window = event_loop.create_window(attrs).expect("Failed to create window");

        // Set WS_EX_TRANSPARENT for mouse passthrough + WS_EX_LAYERED
        #[cfg(windows)]
        set_click_through(&window);

        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
        // Woken by input thread — about_to_wait will drain the channel next.
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Drain all pending input events
        while let Ok(event) = self.input_rx.try_recv() {
            let new_state = process_event(&self.state, &event);
            self.state = new_state;
        }

        // Control visibility and rendering based on state
        match &self.state.drawing {
            DrawingState::Drawing { start, current } => {
                if let Some(window) = &self.window {
                    #[cfg(windows)]
                    draw_border(window, *start, *current, self.border_width, &self.color_mode, &mut self.dib_cache);
                    #[cfg(windows)]
                    show_window_topmost(window);
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    std::time::Instant::now() + std::time::Duration::from_millis(16),
                ));
            }
            _ => {
                if let Some(window) = &self.window {
                    window.set_visible(false);
                    #[cfg(windows)]
                    hide_from_alt_tab(window);
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}

impl App {
    fn render(&mut self) {
        let Some(window) = &self.window else { return; };
        let DrawingState::Drawing { start, current } = &self.state.drawing else { return; };

        #[cfg(windows)]
        {
            draw_border(window, *start, *current, self.border_width, &self.color_mode, &mut self.dib_cache);
            show_window_topmost(window);
        }
    }
}

fn normalize_rect(start: (i32, i32), current: (i32, i32)) -> (i32, i32, i32, i32) {
    let x0 = start.0.min(current.0);
    let y0 = start.1.min(current.1);
    let x1 = start.0.max(current.0);
    let y1 = start.1.max(current.1);
    (x0, y0, x1, y1)
}

/// Get HWND from a winit Window using raw-window-handle 0.6
#[cfg(windows)]
fn get_hwnd(window: &Window) -> windows::Win32::Foundation::HWND {
    let handle = window.window_handle().expect("Failed to get window handle");
    match handle.as_raw() {
        winit::raw_window_handle::RawWindowHandle::Win32(h) => {
            windows::Win32::Foundation::HWND(h.hwnd.get() as *mut core::ffi::c_void)
        }
        _ => panic!("Not a Win32 window"),
    }
}

#[cfg(windows)]
fn set_click_through(window: &Window) {
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex_style
            | WS_EX_TRANSPARENT.0 as isize
            | WS_EX_LAYERED.0 as isize
            | WS_EX_TOPMOST.0 as isize
            | WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TOOLWINDOW.0 as isize,
        );
    }
}

#[cfg(windows)]
fn hide_from_alt_tab(window: &Window) {
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

#[cfg(windows)]
fn show_window_topmost(window: &Window) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let topmost = HWND(-1isize as *mut core::ffi::c_void);
        let _ = SetWindowPos(
            hwnd, topmost, 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Convert HSV to RGB. h: 0-360, s: 0-1, v: 0-1.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

/// Position along rectangle perimeter as fraction 0.0..1.0 (clockwise from top-left).
fn perimeter_position(x: i32, y: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> f32 {
    let w = (x1 - x0) as f32;
    let h = (y1 - y0) as f32;
    let perimeter = 2.0 * (w + h);
    if perimeter == 0.0 { return 0.0; }
    let dx = (x - x0) as f32;
    let dy = (y - y0) as f32;
    let dist = if dy == 0.0 && dx >= 0.0 {
        dx
    } else if dx == w && dy >= 0.0 {
        w + dy
    } else if dy == h && dx >= 0.0 {
        w + h + (w - dx)
    } else {
        2.0 * w + h + (h - dy)
    };
    (dist / perimeter).clamp(0.0, 1.0)
}

fn color_at(x: i32, y: i32, x0: i32, y0: i32, x1: i32, y1: i32, color_mode: &ColorMode, time_offset: f32) -> (u8, u8, u8) {
    match color_mode {
        ColorMode::Solid { r, g, b } => (*r, *g, *b),
        ColorMode::Rainbow => {
            let pos = perimeter_position(x, y, x0, y0, x1, y1);
            let hue = (pos + time_offset).fract() * 360.0;
            hsv_to_rgb(hue, 1.0, 1.0)
        }
    }
}

/// Draw border rectangle using UpdateLayeredWindow with per-pixel alpha.
/// Background is fully transparent (alpha=0), border pixels use configured color/rainbow.
/// Uses cached DIB section to avoid per-frame allocation.
#[cfg(windows)]
fn draw_border(
    window: &Window,
    start: (i32, i32),
    current: (i32, i32),
    border_width: i32,
    color_mode: &ColorMode,
    dib_cache: &mut Option<DibCache>,
) {
    use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);

    unsafe {
        let mut wr = RECT::default();
        if GetWindowRect(hwnd, &mut wr).is_err() {
            return;
        }

        let width = wr.right - wr.left;
        let height = wr.bottom - wr.top;

        if width <= 0 || height <= 0 {
            return;
        }

        // Allocate or resize cached DIB
        match dib_cache {
            Some(cache) => cache.ensure_size(width, height),
            None => {
                *dib_cache = DibCache::new(width, height);
            }
        }
        let cache = match dib_cache {
            Some(c) if !c.pixels.is_null() => c,
            _ => return,
        };

        // Entire window fully transparent
        std::ptr::write_bytes(
            cache.pixels,
            0,
            width as usize * height as usize * 4,
        );

        let elapsed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let time_offset = (elapsed.as_secs_f64() * FLOW_SPEED as f64).fract() as f32;

        // Build a mutable slice over the cached pixel buffer
        let pixel_slice = std::slice::from_raw_parts_mut(
            cache.pixels,
            width as usize * height as usize * 4,
        );

        fill_border_pixels(
            pixel_slice,
            width,
            height,
            wr.left,
            wr.top,
            start,
            current,
            border_width,
            color_mode,
            time_offset,
        );

        let destination = POINT {
            x: wr.left,
            y: wr.top,
        };

        let source = POINT { x: 0, y: 0 };

        let size = SIZE {
            cx: width,
            cy: height,
        };

        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        let screen_dc = GetDC(HWND::default());

        let result = UpdateLayeredWindow(
            hwnd,
            screen_dc,
            Some(&destination),
            Some(&size),
            cache.memory_dc,
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );

        if let Err(error) = result {
            eprintln!("UpdateLayeredWindow failed: {error:?}");
        }

        let _ = ReleaseDC(HWND::default(), screen_dc);
    }
}

/// Create the overlay event loop and proxy. Caller owns the event loop.
pub fn create_event_loop() -> (EventLoop<()>, EventLoopProxy<()>) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let proxy = event_loop.create_proxy();
    (event_loop, proxy)
}

/// Run the overlay event loop on the main thread. Blocks until exit.
pub fn run_overlay(event_loop: EventLoop<()>, input_rx: Receiver<InputEvent>, border_width: i32, color_mode: ColorMode) {
    let mut app = App::new(input_rx, border_width, color_mode);
    event_loop.run_app(&mut app).expect("Event loop error");
}

/// Fill border pixels in a BGRA buffer. Pure logic, no GDI side effects.
/// `buffer` is a BGRA pixel buffer of `width * height` pixels (each 4 bytes).
/// `win_x`, `win_y` are the window position in global screen coordinates.
/// `start`, `current` are the selection rectangle corners in global coordinates.
/// `border_width` is the thickness of the border in pixels.
/// Returns (pixels_written, set_pixel_calls) where set_pixel_calls counts every
/// invocation including overwrites of already-written pixels.
fn fill_border_pixels(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    win_x: i32,
    win_y: i32,
    start: (i32, i32),
    current: (i32, i32),
    border_width: i32,
    color_mode: &ColorMode,
    time_offset: f32,
) -> (usize, usize) {
    if width <= 0 || height <= 0 {
        return (0, 0);
    }

    let (global_x0, global_y0, global_x1, global_y1) = normalize_rect(start, current);

    let x0 = (global_x0 - win_x).clamp(0, width - 1);
    let y0 = (global_y0 - win_y).clamp(0, height - 1);
    let x1 = (global_x1 - win_x).clamp(0, width - 1);
    let y1 = (global_y1 - win_y).clamp(0, height - 1);

    let stride = width as usize * 4;
    let mut unique_written = 0usize;
    let mut total_calls = 0usize;

    let set_pixel = |buf: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8| {
        if x < 0 || x >= width || y < 0 || y >= height {
            return false;
        }
        let offset = y as usize * stride + x as usize * 4;
        let was_zero = buf[offset + 3] == 0;
        buf[offset] = b;
        buf[offset + 1] = g;
        buf[offset + 2] = r;
        buf[offset + 3] = 255;
        was_zero
    };

    if x1 > x0 && y1 > y0 {
        for offset in 0..border_width {
            let top = y0 + offset;
            let bottom = y1 - offset;
            let left = x0 + offset;
            let right = x1 - offset;

            // Top and bottom horizontal edges (full width including corners)
            for x in left..=right {
                let (r, g, b) = color_at(x, top, x0, y0, x1, y1, color_mode, time_offset);
                total_calls += 1;
                if set_pixel(buffer, x, top, r, g, b) {
                    unique_written += 1;
                }
                if top != bottom {
                    let (r, g, b) = color_at(x, bottom, x0, y0, x1, y1, color_mode, time_offset);
                    total_calls += 1;
                    if set_pixel(buffer, x, bottom, r, g, b) {
                        unique_written += 1;
                    }
                }
            }

            // Left and right vertical edges (skip corners already drawn above)
            let inner_top = top + 1;
            let inner_bottom = bottom - 1;
            if inner_top <= inner_bottom {
                for y in inner_top..=inner_bottom {
                    let (r, g, b) = color_at(left, y, x0, y0, x1, y1, color_mode, time_offset);
                    total_calls += 1;
                    if set_pixel(buffer, left, y, r, g, b) {
                        unique_written += 1;
                    }
                    if left != right {
                        let (r, g, b) = color_at(right, y, x0, y0, x1, y1, color_mode, time_offset);
                        total_calls += 1;
                        if set_pixel(buffer, right, y, r, g, b) {
                            unique_written += 1;
                        }
                    }
                }
            }
        }
    }

    (unique_written, total_calls)
}

/// Expected number of unique border pixels for a rect (x0,y0)-(x1,y1) with given border_width.
/// This is the correct count with no double-writes at corners.
fn expected_border_pixel_count(x0: i32, y0: i32, x1: i32, y1: i32, border_width: i32) -> usize {
    let mut count = 0usize;
    for offset in 0..border_width {
        let top = y0 + offset;
        let bottom = y1 - offset;
        let left = x0 + offset;
        let right = x1 - offset;

        if top == bottom {
            // Single row
            count += (right - left + 1) as usize;
        } else if left == right {
            // Single column
            count += (bottom - top + 1) as usize;
        } else {
            // Top edge + bottom edge (full width)
            count += (right - left + 1) as usize * 2;
            // Left + right edges (excluding corners already counted)
            count += (bottom - top - 1) as usize * 2;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rect_swaps_coordinates() {
        assert_eq!(normalize_rect((100, 200), (50, 80)), (50, 80, 100, 200));
    }

    #[test]
    fn normalize_rect_same_point_is_zero_area() {
        assert_eq!(normalize_rect((10, 10), (10, 10)), (10, 10, 10, 10));
    }

    #[test]
    fn normalize_rect_negative_coordinates() {
        assert_eq!(normalize_rect((-100, -200), (50, 80)), (-100, -200, 50, 80));
    }

    #[test]
    fn normalize_rect_already_normalized() {
        assert_eq!(normalize_rect((0, 0), (1920, 1080)), (0, 0, 1920, 1080));
    }

    // -- hsv_to_rgb tests --

    #[test]
    fn hsv_red() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
    }

    #[test]
    fn hsv_green() {
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
    }

    #[test]
    fn hsv_blue() {
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));
    }

    #[test]
    fn hsv_white() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), (255, 255, 255));
    }

    #[test]
    fn hsv_black() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), (0, 0, 0));
    }

    #[test]
    fn hsv_yellow() {
        assert_eq!(hsv_to_rgb(60.0, 1.0, 1.0), (255, 255, 0));
    }

    #[test]
    fn hsv_cyan() {
        assert_eq!(hsv_to_rgb(180.0, 1.0, 1.0), (0, 255, 255));
    }

    #[test]
    fn hsv_magenta() {
        assert_eq!(hsv_to_rgb(300.0, 1.0, 1.0), (255, 0, 255));
    }

    // -- perimeter_position tests --

    #[test]
    fn perimeter_top_left_corner() {
        let pos = perimeter_position(0, 0, 0, 0, 100, 100);
        assert!((pos - 0.0).abs() < 0.001, "expected ~0.0, got {pos}");
    }

    #[test]
    fn perimeter_top_right_corner() {
        let pos = perimeter_position(100, 0, 0, 0, 100, 100);
        assert!((pos - 0.25).abs() < 0.001, "expected ~0.25, got {pos}");
    }

    #[test]
    fn perimeter_bottom_right_corner() {
        let pos = perimeter_position(100, 100, 0, 0, 100, 100);
        assert!((pos - 0.5).abs() < 0.001, "expected ~0.5, got {pos}");
    }

    #[test]
    fn perimeter_bottom_left_corner() {
        let pos = perimeter_position(0, 100, 0, 0, 100, 100);
        assert!((pos - 0.75).abs() < 0.001, "expected ~0.75, got {pos}");
    }

    #[test]
    fn perimeter_mid_top_edge() {
        let pos = perimeter_position(50, 0, 0, 0, 100, 100);
        assert!((pos - 0.125).abs() < 0.001, "expected ~0.125, got {pos}");
    }

    #[test]
    fn perimeter_mid_right_edge() {
        let pos = perimeter_position(100, 50, 0, 0, 100, 100);
        assert!((pos - 0.375).abs() < 0.001, "expected ~0.375, got {pos}");
    }

    // -- fill_border_pixels tests (Bug 1: DIB caching, Bug 2: corner double-write) --

    #[test]
    fn fill_border_can_be_called_repeatedly_on_same_buffer() {
        // Bug 1: Demonstrate that fill_border_pixels operates on a reusable buffer.
        // If DIB caching is implemented, the same buffer should be usable across frames.
        let width: i32 = 200;
        let height: i32 = 200;
        let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
        let mut buffer = vec![0u8; (width * height * 4) as usize];

        // Frame 1: draw at (10,10)-(50,50)
        let (unique1, _) = fill_border_pixels(
            &mut buffer, width, height, 0, 0,
            (10, 10), (50, 50), 4, &color_mode, 0.0,
        );
        assert!(unique1 > 0, "first frame should draw pixels");

        // Clear buffer (simulating transparent background reset, like the real code does)
        buffer.fill(0);

        // Frame 2: draw at (20,20)-(80,80) -- different rect, same buffer
        let (unique2, _) = fill_border_pixels(
            &mut buffer, width, height, 0, 0,
            (20, 20), (80, 80), 4, &color_mode, 0.0,
        );
        assert!(unique2 > 0, "second frame should draw pixels on reused buffer");

        // Verify the second frame's pixels are correct (not corrupted by first frame)
        // Check a pixel that should be in the border of frame 2 but NOT frame 1
        let check_x = 76usize; // right edge of frame 2 border at offset 0
        let check_y = 20usize; // top edge of frame 2
        let offset = check_y * width as usize * 4 + check_x * 4;
        assert_eq!(buffer[offset + 3], 255, "frame 2 border pixel should have alpha=255");
        assert_eq!(buffer[offset + 2], 255, "frame 2 border pixel should have r=255");
    }

    #[test]
    fn fill_border_no_duplicate_pixel_writes_at_corners() {
        // Bug 2: The current implementation writes corner pixels twice (once from
        // horizontal edge loop, once from vertical edge loop). This test verifies
        // that total set_pixel calls equals the number of unique pixels -- no duplicates.
        let width: i32 = 100;
        let height: i32 = 100;
        let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
        let border_width = 4;

        let x0 = 10;
        let y0 = 10;
        let x1 = 90;
        let y1 = 90;

        let mut buffer = vec![0u8; (width * height * 4) as usize];

        let (unique_written, total_calls) = fill_border_pixels(
            &mut buffer, width, height, 0, 0,
            (x0, y0), (x1, y1), border_width, &color_mode, 0.0,
        );

        let expected = expected_border_pixel_count(x0, y0, x1, y1, border_width);

        assert_eq!(
            total_calls, expected,
            "total set_pixel calls ({total_calls}) should equal expected unique count ({expected}); \
             corners are being called with set_pixel multiple times"
        );
        assert_eq!(
            unique_written, expected,
            "unique pixels written ({unique_written}) should equal expected ({expected})"
        );
    }

    #[test]
    fn fill_border_writes_correct_pixel_count_for_small_rect() {
        // Verify the pixel count formula for a small, easily verifiable rect.
        let width: i32 = 50;
        let height: i32 = 50;
        let color_mode = ColorMode::Solid { r: 0, g: 255, b: 0 };
        let border_width = 2;

        let x0 = 5;
        let y0 = 5;
        let x1 = 15;
        let y1 = 15;

        let mut buffer = vec![0u8; (width * height * 4) as usize];

        let (unique_written, total_calls) = fill_border_pixels(
            &mut buffer, width, height, 0, 0,
            (x0, y0), (x1, y1), border_width, &color_mode, 0.0,
        );

        let expected = expected_border_pixel_count(x0, y0, x1, y1, border_width);

        assert_eq!(
            total_calls, expected,
            "small rect total calls mismatch: got {total_calls}, expected {expected}"
        );
        assert_eq!(
            unique_written, expected,
            "small rect unique pixels mismatch: got {unique_written}, expected {expected}"
        );
    }

    #[test]
    fn fill_border_zero_area_rect_writes_nothing() {
        let width: i32 = 50;
        let height: i32 = 50;
        let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
        let mut buffer = vec![0u8; (width * height * 4) as usize];

        let (unique, total) = fill_border_pixels(
            &mut buffer, width, height, 0, 0,
            (10, 10), (10, 10), 4, &color_mode, 0.0,
        );

        assert_eq!(unique, 0, "zero-area rect should write no pixels");
        assert_eq!(total, 0, "zero-area rect should make no set_pixel calls");
        assert!(buffer.iter().all(|&b| b == 0), "buffer should remain all zeros");
    }

    #[test]
    fn fill_border_respects_window_offset() {
        // Verify that window position offset is correctly applied.
        let width: i32 = 100;
        let height: i32 = 100;
        let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
        let mut buffer = vec![0u8; (width * height * 4) as usize];

        // Window at (500,500), rect at (510,510)-(520,520)
        let (unique, _) = fill_border_pixels(
            &mut buffer, width, height, 500, 500,
            (510, 510), (520, 520), 2, &color_mode, 0.0,
        );

        assert!(unique > 0, "should draw pixels within window bounds");

        // The rect (510,510)-(520,520) maps to buffer coords (10,10)-(20,20)
        // Check pixel at (10,10) -- top-left corner of border
        let offset = 10 * width as usize * 4 + 10 * 4;
        assert_eq!(buffer[offset + 3], 255, "pixel at buffer (10,10) should be drawn");
    }
}
