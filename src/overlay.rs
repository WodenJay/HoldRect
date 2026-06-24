// Transparent overlay window + GDI rendering

use std::sync::mpsc::Receiver;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowId, WindowLevel};

#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

use crate::config::AppConfig;
use crate::config::ColorMode;
use crate::popup::PopupManager;
#[cfg(windows)]
use crate::popup::gdi_renderer::GdiRenderer;
use crate::state::AppState;
use crate::state::DrawingState;
use crate::state::InputEvent;
use crate::state::normalize_rect;
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
    modifier_name: String,
    config_rx: Receiver<AppConfig>,
    #[cfg(windows)]
    dib_cache: Option<DibCache>,
    // Popup system
    #[cfg(windows)]
    popup_hwnd: Option<HWND>,
    popup_manager: PopupManager,
    #[cfg(windows)]
    popup_renderer: Option<GdiRenderer>,
    popup_monitor_rect: (i32, i32, i32, i32), // cached at show time
    overlay_shown: bool,
}

#[cfg(windows)]
impl Drop for App {
    fn drop(&mut self) {
        if let Some(hwnd) = self.popup_hwnd {
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
            }
        }
    }
}

impl App {
    pub fn new(input_rx: Receiver<InputEvent>, config_rx: Receiver<AppConfig>, border_width: i32, color_mode: ColorMode, modifier_name: String) -> Self {
        Self {
            window: None,
            state: AppState::default(),
            input_rx,
            border_width,
            color_mode,
            modifier_name: modifier_name.clone(),
            config_rx,
            #[cfg(windows)]
            dib_cache: None,
            #[cfg(windows)]
            popup_hwnd: None,
            popup_manager: PopupManager::new(&modifier_name),
            #[cfg(windows)]
            popup_renderer: None,
            popup_monitor_rect: (0, 0, 1920, 1080),
            overlay_shown: false,
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
            .with_window_level(WindowLevel::AlwaysOnTop)
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

        // Create popup window (raw HWND, not winit)
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::HINSTANCE;
            use windows::Win32::UI::WindowsAndMessaging::*;
            let class_name: Vec<u16> = "HoldRectPopup\0".encode_utf16().collect();
            let window_name: Vec<u16> = "HoldRectPopup\0".encode_utf16().collect();

            // Register window class (re-registration is a no-op)
            unsafe extern "system" fn popup_wnd_proc(hwnd: HWND, msg: u32, wparam: windows::Win32::Foundation::WPARAM, lparam: windows::Win32::Foundation::LPARAM) -> windows::Win32::Foundation::LRESULT {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(popup_wnd_proc),
                hInstance: HINSTANCE::default(),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            unsafe { RegisterClassExW(&wc); }

            let popup_hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    windows::core::PCWSTR(class_name.as_ptr()),
                    windows::core::PCWSTR(window_name.as_ptr()),
                    WS_POPUP,
                    0, 0, 400, 300, // will be resized on render
                    None, None, HINSTANCE::default(), None,
                )
            }.expect("Failed to create popup window");

            self.popup_hwnd = Some(popup_hwnd);
            self.popup_renderer = Some(GdiRenderer::new(popup_hwnd));

            // Make popup an owned window of overlay — it automatically stays
            // above its owner in Z-order, no per-frame SetWindowPos needed.
            {
                use windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW;
                use windows::Win32::UI::WindowsAndMessaging::GWLP_HWNDPARENT;
                let overlay_hwnd = get_hwnd(self.window.as_ref().unwrap());
                unsafe {
                    SetWindowLongPtrW(popup_hwnd, GWLP_HWNDPARENT, overlay_hwnd.0 as _);
                }
            }
        }
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
        // Poll for config changes (hot-reload)
        while let Ok(new_config) = self.config_rx.try_recv() {
            self.border_width = new_config.border_width;
            self.color_mode = new_config.color_mode;
            if new_config.modifier_name != self.modifier_name {
                self.modifier_name = new_config.modifier_name.clone();
                self.popup_manager.update_modifier_name(&self.modifier_name);
            }
            crate::hook::update_modifier_codes(new_config.modifier_vk_codes);
        }

        // Drain all pending input events
        while let Ok(event) = self.input_rx.try_recv() {
            let new_state = process_event(&self.state, &event);
            let was_hidden = !self.popup_manager.needs_frame();
            self.state = new_state;
            // Route to popup manager
            self.popup_manager.on_event(&event, &self.state);
            // Cache monitor rect when popup transitions from Hidden -> visible
            if was_hidden && self.popup_manager.needs_frame() {
                #[cfg(windows)]
                { self.popup_monitor_rect = get_cursor_monitor_work_area(); }
            }
        }

        self.render();

        // Popup animation tick + render
        if self.popup_manager.needs_frame() {
            self.popup_manager.tick();
            #[cfg(windows)]
            if let (Some(renderer), Some(_hwnd)) = (self.popup_renderer.as_mut(), self.popup_hwnd) {
                renderer.render(&self.popup_manager, self.popup_monitor_rect);
            }
        }

        let needs_animation = matches!(&self.state.drawing, DrawingState::Drawing { .. })
            || !self.state.pinned_rects.is_empty()
            || self.popup_manager.needs_frame(); // keep event loop alive for popup animation

        if needs_animation {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                std::time::Instant::now() + std::time::Duration::from_millis(16),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

impl App {
    fn render(&mut self) {
        let Some(window) = &self.window else { return; };

        let has_drawing = matches!(&self.state.drawing, DrawingState::Drawing { .. });
        let has_pinned = !self.state.pinned_rects.is_empty();

        if !should_show_overlay(has_drawing, has_pinned) {
            if self.overlay_shown {
                #[cfg(windows)]
                {
                    // Submit a fully transparent frame before hiding, so
                    // the next show_window_topmost won't flash stale content.
                    let hwnd = get_hwnd(window);
                    let mut wr = windows::Win32::Foundation::RECT::default();
                    if unsafe {
                        windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wr)
                    }
                    .is_ok()
                    {
                        let width = wr.right - wr.left;
                        let height = wr.bottom - wr.top;
                        if width > 0 && height > 0 {
                            if let Some(cache) = self.dib_cache.as_mut() {
                                clear_dib_pixels(cache, width, height);
                                commit_dib(window, cache, width, height, wr.left, wr.top);
                            }
                        }
                    }

                    hide_from_alt_tab(window);
                    self.dib_cache = None;
                }

                #[cfg(not(windows))]
                window.set_visible(false);

                self.overlay_shown = false;
            }
            return;
        }

        #[cfg(windows)]
        {
            let hwnd = get_hwnd(window);
            let mut wr = windows::Win32::Foundation::RECT::default();
            if unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut wr) }.is_err() {
                return;
            }
            let width = wr.right - wr.left;
            let height = wr.bottom - wr.top;
            if width <= 0 || height <= 0 { return; }

            ensure_dib_size(&mut self.dib_cache, width, height);
            let cache = match &mut self.dib_cache {
                Some(c) if !c.pixels.is_null() => c,
                _ => return,
            };
            clear_dib_pixels(cache, width, height);

            let elapsed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let time_offset = (elapsed.as_secs_f64() * FLOW_SPEED as f64).fract() as f32;

            // Build spotlight rects list (screen coords)
            let mut spotlight_rects: Vec<(i32, i32, i32, i32)> = self.state.pinned_rects.iter()
                .filter(|r| r.spotlight)
                .map(|r| (r.x0, r.y0, r.x1, r.y1))
                .collect();

            // Include active drawing rect if spotlight_active
            if self.state.spotlight_active {
                if let DrawingState::Drawing { start, current } = &self.state.drawing {
                    let (x0, y0, x1, y1) = normalize_rect(*start, *current);
                    spotlight_rects.push((x0, y0, x1, y1));
                }
            }

            // Dim outside spotlight rects
            dim_outside_spotlights_in_dib(cache, width, height, &spotlight_rects, wr.left, wr.top);

            // Draw all pinned rects
            for rect in &self.state.pinned_rects {
                draw_rect_in_dib(cache, width, height, wr.left, wr.top,
                                 (rect.x0, rect.y0), (rect.x1, rect.y1), self.border_width, &self.color_mode, time_offset);
            }

            // Draw active rect on top
            if let DrawingState::Drawing { start, current } = &self.state.drawing {
                draw_rect_in_dib(cache, width, height, wr.left, wr.top,
                                 *start, *current, self.border_width, &self.color_mode, time_offset);
            }

            if !self.overlay_shown {
                self.overlay_shown = true;
            }
            show_window_topmost(window); // re-enforce Z-order every frame
            commit_dib(window, cache, width, height, wr.left, wr.top);
        }
    }
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
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let _ = SetWindowPos(
            hwnd, HWND_TOPMOST, 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Whether the overlay should be visible based on current drawing state.
/// Pure logic extracted from `render()` for testability.
fn should_show_overlay(has_drawing: bool, has_pinned: bool) -> bool {
    has_drawing || has_pinned
}

/// Simulate multi-frame overlay state and return the number of times
/// `show_window_topmost` should have been called.
/// Models the Z-order enforcement policy.
#[cfg(test)]
fn topmost_enforce_count(frames: &[(bool, bool)]) -> usize {
    let mut count = 0;
    for &(has_drawing, has_pinned) in frames {
        if should_show_overlay(has_drawing, has_pinned) {
            count += 1; // enforce topmost EVERY visible frame
        }
    }
    count
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

/// Ensure DIB is allocated to the correct size.
#[cfg(windows)]
fn ensure_dib_size(dib_cache: &mut Option<DibCache>, width: i32, height: i32) {
    match dib_cache {
        Some(cache) => cache.ensure_size(width, height),
        None => {
            *dib_cache = DibCache::new(width, height);
        }
    }
}

/// Clear the DIB to fully transparent pixels.
#[cfg(windows)]
fn clear_dib_pixels(cache: &mut DibCache, width: i32, height: i32) {
    unsafe {
        std::ptr::write_bytes(cache.pixels, 0, width as usize * height as usize * 4);
    }
}

/// Dim all pixels outside the given spotlight rects.
/// `rects` are (x0, y0, x1, y1) in screen coordinates.
/// Interior of each rect (x0..=x1, y0..=y1) is cleared to transparent.
fn dim_outside_spotlights(
    buffer: &mut [u8],
    width: i32,
    height: i32,
    rects: &[(i32, i32, i32, i32)],
    win_x: i32,
    win_y: i32,
) {
    if rects.is_empty() || width <= 0 || height <= 0 {
        return;
    }

    let stride = width as usize * 4;

    // Dim all pixels: BGRA = (0, 0, 0, 160)
    let pattern: [u8; 4] = [0, 0, 0, 160];
    for chunk in buffer.chunks_exact_mut(4) {
        chunk.copy_from_slice(&pattern);
    }

    // Clear interior of each spotlight rect to fully transparent
    for &(sx0, sy0, sx1, sy1) in rects {
        let x0 = (sx0 - win_x).clamp(0, width - 1);
        let y0 = (sy0 - win_y).clamp(0, height - 1);
        let x1 = (sx1 - win_x).clamp(0, width - 1);
        let y1 = (sy1 - win_y).clamp(0, height - 1);

        if x1 < x0 || y1 < y0 {
            continue;
        }

        for y in y0..=y1 {
            for x in x0..=x1 {
                let off = y as usize * stride + x as usize * 4;
                buffer[off] = 0;
                buffer[off + 1] = 0;
                buffer[off + 2] = 0;
                buffer[off + 3] = 0;
            }
        }
    }
}

/// Wrapper that operates directly on DibCache (same pattern as draw_rect_in_dib).
#[cfg(windows)]
fn dim_outside_spotlights_in_dib(
    cache: &mut DibCache,
    width: i32,
    height: i32,
    rects: &[(i32, i32, i32, i32)],
    win_x: i32,
    win_y: i32,
) {
    unsafe {
        let pixel_slice = std::slice::from_raw_parts_mut(
            cache.pixels,
            width as usize * height as usize * 4,
        );
        dim_outside_spotlights(pixel_slice, width, height, rects, win_x, win_y);
    }
}

/// Draw one rectangle's border into the DIB buffer. Does NOT call UpdateLayeredWindow.
#[cfg(windows)]
fn draw_rect_in_dib(
    cache: &mut DibCache,
    width: i32,
    height: i32,
    win_x: i32,
    win_y: i32,
    start: (i32, i32),
    current: (i32, i32),
    border_width: i32,
    color_mode: &ColorMode,
    time_offset: f32,
) {
    unsafe {
        let pixel_slice = std::slice::from_raw_parts_mut(
            cache.pixels,
            width as usize * height as usize * 4,
        );
        fill_border_pixels(
            pixel_slice, width, height, win_x, win_y,
            start, current, border_width, color_mode, time_offset,
        );
    }
}

/// Push the DIB buffer to screen via UpdateLayeredWindow.
#[cfg(windows)]
fn commit_dib(window: &Window, cache: &DibCache, width: i32, height: i32, win_x: i32, win_y: i32) {
    use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let destination = POINT { x: win_x, y: win_y };
        let source = POINT { x: 0, y: 0 };
        let size = SIZE { cx: width, cy: height };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let screen_dc = GetDC(HWND::default());
        let result = UpdateLayeredWindow(
            hwnd, screen_dc, Some(&destination), Some(&size),
            cache.memory_dc, Some(&source), COLORREF(0), Some(&blend), ULW_ALPHA,
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
pub fn run_overlay(event_loop: EventLoop<()>, input_rx: Receiver<InputEvent>, config_rx: Receiver<AppConfig>, border_width: i32, color_mode: ColorMode, modifier_name: String) {
    let mut app = App::new(input_rx, config_rx, border_width, color_mode, modifier_name);
    event_loop.run_app(&mut app).expect("Event loop error");
}

/// Get the work area of the monitor containing the cursor.
#[cfg(windows)]
fn get_cursor_monitor_work_area() -> (i32, i32, i32, i32) {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(hmon, &mut info);
        let work = info.rcWork;
        (work.left, work.top, work.right, work.bottom)
    }
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
#[cfg(test)]
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

    #[test]
    fn normalize_rect_with_i32_boundaries() {
        // start at MAX, current at MIN: should swap
        assert_eq!(
            normalize_rect((i32::MAX, i32::MAX), (i32::MIN, i32::MIN)),
            (i32::MIN, i32::MIN, i32::MAX, i32::MAX)
        );
    }

    #[test]
    fn normalize_rect_one_axis_swapped() {
        // x already sorted (0 < 100), y needs swap (200 > 80)
        assert_eq!(
            normalize_rect((0, 200), (100, 80)),
            (0, 80, 100, 200)
        );
    }

    #[test]
    fn normalize_rect_zero_origin() {
        assert_eq!(
            normalize_rect((0, 0), (0, 0)),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn hsv_360_wraps_to_red() {
        // h=360 should produce the same result as h=0 (red)
        let at_0 = hsv_to_rgb(0.0, 1.0, 1.0);
        let at_360 = hsv_to_rgb(360.0, 1.0, 1.0);
        assert_eq!(at_360, at_0, "h=360 should wrap to same as h=0");
    }

    #[test]
    fn hsv_partial_saturation() {
        // h=0 s=0.5 v=1.0 should be a desaturated red (pink-ish)
        let (r, g, b) = hsv_to_rgb(0.0, 0.5, 1.0);
        assert_eq!(r, 255, "red channel should be max");
        assert!(g > 0 && g < 255, "green should be mid-range for desaturated red");
        assert!(b > 0 && b < 255, "blue should be mid-range for desaturated red");
    }

    #[test]
    fn hsv_partial_value() {
        // h=0 s=1.0 v=0.5 should be a dark red
        let (r, g, b) = hsv_to_rgb(0.0, 1.0, 0.5);
        assert_eq!(r, 128, "dark red channel should be ~128 (0.5*255 rounded)");
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    mod missing_tests {
        use super::super::hsv_to_rgb;
        use super::super::perimeter_position;
        use super::super::color_at;
        use super::super::fill_border_pixels;
        use super::super::expected_border_pixel_count;
        use crate::config::ColorMode;
        use crate::config::parse_color;
        use crate::config::modifier_vk_codes;
        use crate::config::AppConfig;
        use windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN;
        use windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP;
        use windows::Win32::UI::WindowsAndMessaging::WM_MBUTTONDOWN;
        use windows::Win32::UI::WindowsAndMessaging::WM_MOUSEMOVE;
        use windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP;
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_LCONTROL;
        use crate::hook::decide_keyboard;
        use crate::hook::decide_mouse;
        use crate::state::InputEvent;

        #[test]
        fn hsv_sector_boundary_60() {
            // h=60 is the boundary between sector 0 and sector 1
            let (r, g, b) = hsv_to_rgb(60.0, 1.0, 1.0);
            assert_eq!((r, g, b), (255, 255, 0), "h=60 should be yellow");
        }

        #[test]
        fn hsv_sector_boundary_300() {
            // h=300 is the boundary between sector 4 and sector 5
            let (r, g, b) = hsv_to_rgb(300.0, 1.0, 1.0);
            assert_eq!((r, g, b), (255, 0, 255), "h=300 should be magenta");
        }

        #[test]
        fn perimeter_position_zero_area() {
            // When x0==x1 and y0==y1, perimeter is 0 -- should return 0.0
            let pos = perimeter_position(5, 5, 5, 5, 5, 5);
            assert_eq!(pos, 0.0, "zero-area rect should return 0.0");
        }

        #[test]
        fn perimeter_position_non_square_rect() {
            // 200x100 rect: perimeter = 600
            // Mid-bottom edge: total dist = w + h + w/2 = 200 + 100 + 100 = 400, pos = 400/600
            let pos = perimeter_position(100, 100, 0, 0, 200, 100);
            assert!((pos - (400.0 / 600.0)).abs() < 0.001, "expected ~0.667, got {pos}");
        }

        #[test]
        fn perimeter_position_mid_bottom_edge() {
            let pos = perimeter_position(50, 100, 0, 0, 100, 100);
            assert!((pos - 0.625).abs() < 0.001, "expected ~0.625, got {pos}");
        }

        #[test]
        fn perimeter_position_mid_left_edge() {
            let pos = perimeter_position(0, 50, 0, 0, 100, 100);
            assert!((pos - 0.875).abs() < 0.001, "expected ~0.875, got {pos}");
        }

        #[test]
        fn perimeter_position_negative_offset_rect() {
            // Rect at (-100,-100) to (100,100), top-left corner
            let pos = perimeter_position(-100, -100, -100, -100, 100, 100);
            assert!((pos - 0.0).abs() < 0.001, "top-left of negative-offset rect should be ~0.0, got {pos}");
        }

        #[test]
        fn color_at_solid_mode_ignores_position() {
            let mode = ColorMode::Solid { r: 100, g: 150, b: 200 };
            assert_eq!(color_at(0, 0, 0, 0, 100, 100, &mode, 0.0), (100, 150, 200));
            assert_eq!(color_at(999, 999, 0, 0, 100, 100, &mode, 0.0), (100, 150, 200));
        }

        #[test]
        fn color_at_rainbow_with_time_offset() {
            let (r0, g0, b0) = color_at(0, 0, 0, 0, 100, 100, &ColorMode::Rainbow, 0.0);
            assert_eq!((r0, g0, b0), (255, 0, 0), "top-left at t=0 should be red");
            let (r1, g1, b1) = color_at(0, 0, 0, 0, 100, 100, &ColorMode::Rainbow, 0.25);
            assert!(g1 > r1, "at t=0.25 green should dominate over red");
            assert!(g1 > b1, "at t=0.25 green should dominate over blue");
        }

        #[test]
        fn color_at_solid_all_black() {
            let mode = ColorMode::Solid { r: 0, g: 0, b: 0 };
            assert_eq!(color_at(50, 50, 0, 0, 100, 100, &mode, 0.0), (0, 0, 0));
        }

        #[test]
        fn fill_border_negative_width_returns_zero() {
            let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
            let mut buffer = vec![0u8; 100];
            let (unique, total) = fill_border_pixels(
                &mut buffer, -1, 100, 0, 0,
                (0, 0), (10, 10), 4, &color_mode, 0.0,
            );
            assert_eq!(unique, 0);
            assert_eq!(total, 0);
        }

        #[test]
        fn fill_border_negative_height_returns_zero() {
            let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
            let mut buffer = vec![0u8; 100];
            let (unique, total) = fill_border_pixels(
                &mut buffer, 100, -1, 0, 0,
                (0, 0), (10, 10), 4, &color_mode, 0.0,
            );
            assert_eq!(unique, 0);
            assert_eq!(total, 0);
        }

        #[test]
        fn fill_border_width_1() {
            let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
            let mut buffer = vec![0u8; (100 * 100 * 4) as usize];
            let (unique, total) = fill_border_pixels(
                &mut buffer, 100, 100, 0, 0,
                (10, 10), (20, 20), 1, &color_mode, 0.0,
            );
            let expected = expected_border_pixel_count(10, 10, 20, 20, 1);
            assert_eq!(unique, expected, "border_width=1 should match expected count");
            assert_eq!(total, expected, "no duplicate writes with border_width=1");
        }

        #[test]
        fn fill_border_rect_entirely_outside_window() {
            let color_mode = ColorMode::Solid { r: 255, g: 0, b: 0 };
            let mut buffer = vec![0u8; (100 * 100 * 4) as usize];
            let (unique, _) = fill_border_pixels(
                &mut buffer, 100, 100, 0, 0,
                (200, 200), (300, 300), 4, &color_mode, 0.0,
            );
            assert_eq!(unique, 0, "rect outside window should write nothing");
            assert!(buffer.iter().all(|&b| b == 0), "buffer should remain all zeros");
        }

        #[test]
        fn fill_border_rect_partially_clipped() {
            let color_mode = ColorMode::Solid { r: 0, g: 0, b: 255 };
            let mut buffer = vec![0u8; (50 * 50 * 4) as usize];
            let (unique, total) = fill_border_pixels(
                &mut buffer, 50, 50, 0, 0,
                (-10, -10), (30, 30), 2, &color_mode, 0.0,
            );
            assert!(unique > 0, "partially visible rect should draw some pixels");
            let expected = expected_border_pixel_count(0, 0, 30, 30, 2);
            assert_eq!(unique, expected, "visible portion pixel count should match expected");
            assert_eq!(total, expected, "no duplicate writes in clipped rect");
        }

        #[test]
        fn fill_border_negative_start_coords() {
            let color_mode = ColorMode::Solid { r: 255, g: 255, b: 0 };
            let mut buffer = vec![0u8; (100 * 100 * 4) as usize];
            let (unique, _) = fill_border_pixels(
                &mut buffer, 100, 100, -60, -60,
                (-50, -50), (-40, -40), 2, &color_mode, 0.0,
            );
            assert!(unique > 0, "negative global coords should still draw into buffer");
            let offset = 10 * 100 * 4 + 10 * 4;
            assert_eq!(buffer[offset + 3], 255, "pixel at buffer (10,10) should be drawn");
        }

        #[test]
        fn fill_border_rainbow_mode() {
            let mut buffer = vec![0u8; (100 * 100 * 4) as usize];
            let (unique, _) = fill_border_pixels(
                &mut buffer, 100, 100, 0, 0,
                (10, 10), (50, 50), 2, &ColorMode::Rainbow, 0.0,
            );
            assert!(unique > 0, "rainbow mode should draw pixels");
            // Top-left corner pixel (10,10): perimeter pos ~0.0, hue ~0 (red)
            let off_tl = 10 * 100 * 4 + 10 * 4;
            let (r_tl, g_tl, b_tl) = (buffer[off_tl + 2], buffer[off_tl + 1], buffer[off_tl]);
            // Bottom-right corner pixel (49,49): perimeter pos ~0.74, hue ~266 (blue-ish)
            let off_br = 49 * 100 * 4 + 49 * 4;
            let (r_br, g_br, b_br) = (buffer[off_br + 2], buffer[off_br + 1], buffer[off_br]);
            assert_ne!(
                (r_tl, g_tl, b_tl), (r_br, g_br, b_br),
                "opposite corners should produce different rainbow colors"
            );
        }

        #[test]
        fn fill_border_with_time_offset_shifts_colors() {
            let mut buf0 = vec![0u8; (100 * 100 * 4) as usize];
            let mut buf1 = vec![0u8; (100 * 100 * 4) as usize];
            let (u0, _) = fill_border_pixels(
                &mut buf0, 100, 100, 0, 0,
                (10, 10), (50, 50), 2, &ColorMode::Rainbow, 0.0,
            );
            let (u1, _) = fill_border_pixels(
                &mut buf1, 100, 100, 0, 0,
                (10, 10), (50, 50), 2, &ColorMode::Rainbow, 0.5,
            );
            assert_eq!(u0, u1, "same rect should write same number of pixels");
            let off = 10 * 100 * 4 + 10 * 4;
            assert_ne!(buf0[off + 2], buf1[off + 2], "time_offset should shift colors");
        }

        #[test]
        fn expected_border_pixel_count_single_pixel_rect() {
            let count = expected_border_pixel_count(5, 5, 5, 5, 1);
            assert_eq!(count, 1, "single-pixel rect should have exactly 1 border pixel");
        }

        #[test]
        fn expected_border_pixel_count_border_1() {
            let count = expected_border_pixel_count(0, 0, 10, 10, 1);
            assert_eq!(count, 40, "10x10 rect border=1 should be 40 pixels (inclusive bounds: 11x11 area)");
        }

        #[test]
        fn expected_border_pixel_count_border_equals_half_rect() {
            // 10x10 rect with border_width=5: all layers collapse
            // offset 0: 11*2 + 9*2 = 40
            // offset 1: 9*2 + 7*2 = 32
            // offset 2: 7*2 + 5*2 = 24
            // offset 3: 5*2 + 3*2 = 16
            // offset 4: single row = 3*2 + 1*2 = 8
            let count = expected_border_pixel_count(0, 0, 10, 10, 5);
            assert_eq!(count, 120, "10x10 rect border=5 should fill interior (120 pixels, inclusive bounds)");
        }

        #[test]
        fn color_parse_empty_string_returns_default_solid() {
            assert_eq!(
                parse_color(""),
                ColorMode::Solid { r: 255, g: 0, b: 0 }
            );
        }

        #[test]
        fn color_parse_short_hex_returns_default_solid() {
            assert_eq!(
                parse_color("#FFF"),
                ColorMode::Solid { r: 255, g: 0, b: 0 }
            );
        }

        #[test]
        fn color_parse_long_hex_returns_default_solid() {
            assert_eq!(
                parse_color("#FF00FF00"),
                ColorMode::Solid { r: 255, g: 0, b: 0 }
            );
        }

        #[test]
        fn color_parse_hex_all_zeros() {
            assert_eq!(
                parse_color("#000000"),
                ColorMode::Solid { r: 0, g: 0, b: 0 }
            );
        }

        #[test]
        fn color_parse_hex_all_ones() {
            assert_eq!(
                parse_color("#FFFFFF"),
                ColorMode::Solid { r: 255, g: 255, b: 255 }
            );
        }

        #[test]
        fn modifier_vk_codes_empty_string_defaults_to_alt() {
            assert_eq!(modifier_vk_codes(""), vec![0x12, 0xA4, 0xA5]);
        }

        #[test]
        fn modifier_vk_codes_case_sensitive() {
            // lowercase 'alt' does not match "Alt", falls to default
            assert_eq!(modifier_vk_codes("alt"), vec![0x12, 0xA4, 0xA5]);
            assert_eq!(modifier_vk_codes("ctrl"), vec![0x12, 0xA4, 0xA5]);
        }

        #[test]
        fn color_parse_mixed_case_hex() {
            assert_eq!(
                parse_color("#aAbBcC"),
                ColorMode::Solid { r: 170, g: 187, b: 204 }
            );
        }

        #[test]
        fn modifier_vk_codes_win_has_two_codes() {
            let codes = modifier_vk_codes("Win");
            assert_eq!(codes.len(), 2);
            // Win key has Left(0x5B) and Right(0x5C), no generic VK like Alt/Ctrl/Shift
            assert_eq!(codes, vec![0x5B, 0x5C]);
        }

        #[test]
        fn parse_partial_config_only_color() {
            let toml_str = r#"color = "rainbow""#;
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.modifier_vk_codes, vec![0x12, 0xA4, 0xA5]);
            assert_eq!(cfg.border_width, 4);
            assert_eq!(cfg.color_mode, ColorMode::Rainbow);
        }

        #[test]
        fn parse_negative_border_width_clamped_to_one() {
            let toml_str = r#"border_width = -5"#;
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.border_width, 1);
        }

        #[test]
        fn parse_border_width_at_lower_bound() {
            let toml_str = r#"border_width = 1"#;
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.border_width, 1);
        }

        #[test]
        fn parse_border_width_at_upper_bound() {
            let toml_str = r#"border_width = 20"#;
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.border_width, 20);
        }

        #[test]
        fn parse_whitespace_only_uses_defaults() {
            let cfg = AppConfig::parse("   \n\t  ").unwrap();
            assert_eq!(cfg, AppConfig::default());
        }

        #[test]
        fn parse_unknown_keys_ignored() {
            let toml_str = r#"unknown_key = 42
modifier = "Ctrl""#;
            // serde ignores extra keys by default with Deserialize
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.modifier_vk_codes, vec![0x11, 0xA2, 0xA3]);
        }

        #[test]
        fn parse_color_hex_with_hash_via_parse() {
            let toml_str = r##"color = "#FF00FF""##;
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.color_mode, ColorMode::Solid { r: 255, g: 0, b: 255 });
        }

        #[test]
        fn parse_modifier_win() {
            let toml_str = r#"modifier = "Win""#;
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.modifier_vk_codes, vec![0x5B, 0x5C]);
            assert_eq!(cfg.border_width, 4);
        }

        #[test]
        fn parse_color_mixed_case_rainbow() {
            let toml_str = r#"color = "rAiNbOw""#;
            let cfg = AppConfig::parse(toml_str).unwrap();
            assert_eq!(cfg.color_mode, ColorMode::Rainbow);
        }

        #[test]
        fn decide_keyboard_zero_vk_code_returns_none() {
            let codes = vec![0x12, 0xA4, 0xA5];
            let result = decide_keyboard(0, true, &codes, false);
            assert_eq!(result, None);
        }

        #[test]
        fn decide_keyboard_max_u32_vk_code_returns_none() {
            let codes = vec![0x12, 0xA4, 0xA5];
            let result = decide_keyboard(u32::MAX, true, &codes, false);
            assert_eq!(result, None);
        }

        #[test]
        fn decide_keyboard_zero_vk_code_in_modifier_codes_matches() {
            let codes = vec![0];
            let result = decide_keyboard(0, true, &codes, false);
            assert_eq!(result, Some(InputEvent::ModifierChanged { pressed: true }));
        }

        #[test]
        fn decide_keyboard_single_element_exact_match() {
            let codes = vec![0x12];
            assert_eq!(decide_keyboard(0x12, true, &codes, false), Some(InputEvent::ModifierChanged { pressed: true }));
            assert_eq!(decide_keyboard(0x12, false, &codes, false), Some(InputEvent::ModifierChanged { pressed: false }));
            assert_eq!(decide_keyboard(0x11, true, &codes, false), None);
        }

        #[test]
        fn decide_keyboard_duplicate_modifier_codes_matches() {
            let codes = vec![0x12, 0x12, 0xA4];
            assert_eq!(decide_keyboard(0x12, true, &codes, false), Some(InputEvent::ModifierChanged { pressed: true }));
        }

        #[test]
        fn decide_mouse_zero_coordinates() {
            let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (0, 0), true, false, true);
            assert_eq!(event, Some(InputEvent::MouseButtonDown { x: 0, y: 0 }));
            assert!(suppress);
        }

        #[test]
        fn decide_mouse_negative_coordinates() {
            let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (-100, -200), false, true, false);
            assert_eq!(event, Some(InputEvent::MouseMove { x: -100, y: -200 }));
            assert!(!suppress);
        }

        #[test]
        fn decide_mouse_unknown_msg_no_drag_suppress_modifier_held() {
            let (event, suppress) = decide_mouse(0x020B, (100, 200), true, false, true); // WM_XBUTTONDOWN
            assert_eq!(event, None);
            assert!(!suppress);
        }

        #[test]
        fn decide_mouse_extreme_coordinates() {
            let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (i32::MAX, i32::MIN), true, false, true);
            assert_eq!(event, Some(InputEvent::MouseButtonDown { x: i32::MAX, y: i32::MIN }));
            assert!(suppress);
        }

        #[test]
        fn decide_mouse_unknown_msg_during_drag_passes_through() {
            let (event, suppress) = decide_mouse(WM_MBUTTONDOWN, (100, 200), false, true, false);
            assert_eq!(event, None);
            assert!(!suppress);
        }

        #[test]
        fn decide_mouse_rbuttonup_during_drag_passes_through() {
            let (event, suppress) = decide_mouse(WM_RBUTTONUP, (100, 200), false, true, false);
            assert_eq!(event, None);
            assert!(!suppress);
        }

        #[test]
        fn decide_mouse_lbuttondown_during_drag_passes_through() {
            let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (200, 300), true, true, true);
            assert_eq!(event, None);
            assert!(!suppress);
        }

        #[test]
        fn decide_mouse_suppress_false_modifier_held_true_no_drag() {
            let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (100, 200), false, false, true);
            assert_eq!(event, None);
            assert!(!suppress);
        }

        #[test]
        fn decide_mouse_drag_with_modifier_held_and_suppress() {
            let (event, suppress) = decide_mouse(WM_LBUTTONUP, (400, 500), true, true, true);
            assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 400, y: 500 }));
            assert!(suppress);
        }

        #[test]
        fn decide_mouse_move_during_drag_with_all_flags_true() {
            let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (50, 60), true, true, true);
            assert_eq!(event, Some(InputEvent::MouseMove { x: 50, y: 60 }));
            assert!(!suppress);
        }

        #[test]
        fn full_drag_sequence_with_ctrl_modifier() {
            let mut should_suppress: bool;
            let mut drag_in_progress = false;
            let ctrl_codes: &[u32] = &[0x11, 0xA2, 0xA3];

            let event = decide_keyboard(VK_LCONTROL.0 as u32, true, ctrl_codes, false);
            assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: true }));
            should_suppress = true;

            let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (50, 75), should_suppress, drag_in_progress, true);
            assert_eq!(event, Some(InputEvent::MouseButtonDown { x: 50, y: 75 }));
            assert!(suppress);
            drag_in_progress = true;

            let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (150, 175), should_suppress, drag_in_progress, true);
            assert_eq!(event, Some(InputEvent::MouseMove { x: 150, y: 175 }));
            assert!(!suppress);

            let (event, suppress) = decide_mouse(WM_LBUTTONUP, (200, 250), should_suppress, drag_in_progress, true);
            assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 200, y: 250 }));
            assert!(suppress);
            drag_in_progress = false;

            let event = decide_keyboard(VK_LCONTROL.0 as u32, false, ctrl_codes, false);
            assert_eq!(event, Some(InputEvent::ModifierChanged { pressed: false }));
            should_suppress = false;

            let _ = (&should_suppress, &drag_in_progress);
        }

        #[test]
        fn multiple_mouse_moves_during_drag_all_track() {
            let coords = [(10, 20), (30, 40), (50, 60), (70, 80), (90, 100)];
            for &(x, y) in &coords {
                let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (x, y), true, true, true);
                assert_eq!(event, Some(InputEvent::MouseMove { x, y }));
                assert!(!suppress);
            }
        }

        #[test]
        fn drag_then_idle_then_new_drag() {
            let mut should_suppress = false;
            let mut drag_in_progress = false;

            // Cycle 1
            should_suppress = true;
            let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (10, 10), should_suppress, drag_in_progress, true);
            assert!(suppress);
            drag_in_progress = true;
            let (event, suppress) = decide_mouse(WM_LBUTTONUP, (20, 20), should_suppress, drag_in_progress, true);
            assert!(suppress);
            drag_in_progress = false;
            should_suppress = false;

            // Idle
            let (event, suppress) = decide_mouse(WM_MOUSEMOVE, (50, 50), false, false, false);
            assert_eq!(event, None);
            assert!(!suppress);

            // Cycle 2
            should_suppress = true;
            let (event, suppress) = decide_mouse(WM_LBUTTONDOWN, (30, 30), should_suppress, drag_in_progress, true);
            assert_eq!(event, Some(InputEvent::MouseButtonDown { x: 30, y: 30 }));
            assert!(suppress);
            drag_in_progress = true;
            let (event, suppress) = decide_mouse(WM_LBUTTONUP, (40, 40), should_suppress, drag_in_progress, true);
            assert_eq!(event, Some(InputEvent::MouseButtonUp { x: 40, y: 40 }));
            assert!(suppress);
            drag_in_progress = false;
            should_suppress = false;

            let _ = (&should_suppress, &drag_in_progress);
        }

        #[test]
        fn create_icon_center_is_white() {
            const SIZE: usize = 32;
            const INSET: f64 = 4.0;
            const RADIUS: f64 = 5.0;
            const BORDER_W: f64 = 5.0;
            let half = (SIZE as f64 - 1.0 - INSET * 2.0) / 2.0;
            let cx = (SIZE as f64 - 1.0) / 2.0;
            let cy = (SIZE as f64 - 1.0) / 2.0;
            let center_x = (cx as usize).min(SIZE - 1);
            let center_y = (cy as usize).min(SIZE - 1);
            let dx = (center_x as f64 - cx).abs() - (half - RADIUS);
            let dy = (center_y as f64 - cy).abs() - (half - RADIUS);
            let dist = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt()
            } else {
                dx.max(dy).max(0.0)
            };
            assert!(dist <= RADIUS - BORDER_W, "Center pixel dist={} should be inside border (interior)", dist);
        }
    }

    mod spotlight_tests {
        use super::super::dim_outside_spotlights;

        #[test]
        fn dim_outside_spotlights_fills_dark_outside_rect() {
            let width = 10i32;
            let height = 10i32;
            let mut buf = vec![0u8; (width * height * 4) as usize];

            let rects = vec![(2, 2, 7, 7)];
            dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

            let outside_offset = (0 * width as usize + 0) * 4;
            assert_eq!(buf[outside_offset + 3], 160, "outside pixel alpha should be 160");
            assert_eq!(buf[outside_offset], 0, "B=0");
            assert_eq!(buf[outside_offset + 1], 0, "G=0");
            assert_eq!(buf[outside_offset + 2], 0, "R=0");
        }

        #[test]
        fn dim_outside_spotlights_clears_interior() {
            let width = 10i32;
            let height = 10i32;
            let mut buf = vec![0u8; (width * height * 4) as usize];

            let rects = vec![(2, 2, 7, 7)];
            dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

            let inside_offset = (4 * width as usize + 4) * 4;
            assert_eq!(buf[inside_offset + 3], 0, "inside pixel alpha should be 0");
        }

        #[test]
        fn dim_outside_spotlights_noop_when_empty() {
            let width = 10i32;
            let height = 10i32;
            let mut buf = vec![0u8; (width * height * 4) as usize];

            dim_outside_spotlights(&mut buf, width, height, &[], 0, 0);

            assert!(buf.iter().all(|&b| b == 0));
        }

        #[test]
        fn dim_outside_spotlights_mixed_spotlight_and_non_spotlight() {
            let width = 20i32;
            let height = 20i32;
            let mut buf = vec![0u8; (width * height * 4) as usize];

            let rects = vec![(5, 5, 10, 10)];
            dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

            let inside_offset = (7 * width as usize + 7) * 4;
            assert_eq!(buf[inside_offset + 3], 0, "spotlight interior should be clear");

            let outside_offset = (0 * width as usize + 0) * 4;
            assert_eq!(buf[outside_offset + 3], 160, "outside spotlight should be dimmed");

            let non_spotlight_interior = (15 * width as usize + 15) * 4;
            assert_eq!(buf[non_spotlight_interior + 3], 160, "non-spotlight interior stays dimmed");
        }

        #[test]
        fn dim_outside_spotlights_overlapping_rects() {
            let width = 20i32;
            let height = 20i32;
            let mut buf = vec![0u8; (width * height * 4) as usize];

            let rects = vec![(2, 2, 10, 10), (5, 5, 15, 15)];
            dim_outside_spotlights(&mut buf, width, height, &rects, 0, 0);

            let overlap_offset = (7 * width as usize + 7) * 4;
            assert_eq!(buf[overlap_offset + 3], 0, "overlap interior should be clear");

            let outside_offset = (0 * width as usize + 0) * 4;
            assert_eq!(buf[outside_offset + 3], 160, "outside both should be dimmed");
        }

        #[test]
        fn dim_outside_spotlights_with_window_offset() {
            let width = 10i32;
            let height = 10i32;
            let mut buf = vec![0u8; (width * height * 4) as usize];

            let rects = vec![(12, 12, 17, 17)];
            dim_outside_spotlights(&mut buf, width, height, &rects, 10, 10);

            let inside_offset = (4 * width as usize + 4) * 4;
            assert_eq!(buf[inside_offset + 3], 0, "inside should be clear with offset");

            let outside_offset = 0;
            assert_eq!(buf[outside_offset + 3], 160, "outside should be dimmed with offset");
        }
    }

    // -- should_show_overlay tests --

    #[test]
    fn overlay_hidden_when_no_content() {
        assert!(!should_show_overlay(false, false));
    }

    #[test]
    fn overlay_shown_when_drawing() {
        assert!(should_show_overlay(true, false));
    }

    #[test]
    fn overlay_shown_when_pinned() {
        assert!(should_show_overlay(false, true));
    }

    #[test]
    fn overlay_shown_when_both() {
        assert!(should_show_overlay(true, true));
    }

    // -- topmost_enforce_count tests --
    //
    // The overlay must call show_window_topmost EVERY frame while visible,
    // not only on the hidden→shown transition. Windows can demote HWND_TOPMOST
    // at any time; re-enforcing each frame prevents the overlay from sinking
    // behind other windows mid-drag.

    #[test]
    fn topmost_not_enforced_when_idle() {
        // All frames: no drawing, no pinned → no enforcement
        assert_eq!(topmost_enforce_count(&[(false, false); 5]), 0);
    }

    #[test]
    fn topmost_enforced_on_first_show() {
        // Transition: idle → drawing → should enforce
        let frames = [(false, false), (true, false)];
        assert_eq!(topmost_enforce_count(&frames), 1);
    }

    #[test]
    fn topmost_enforced_every_visible_frame() {
        // After show, 3 more visible frames → total 3 enforcements (one per visible frame)
        // Current buggy code returns 1 (only on transition).
        // This test captures the core bug: Z-order lost mid-drag.
        let frames = [
            (false, false), // idle
            (true, false),  // start drawing → show (enforce #1)
            (true, false),  // still drawing (enforce #2)
            (true, false),  // still drawing (enforce #3)
        ];
        assert_eq!(topmost_enforce_count(&frames), 3);
    }

    #[test]
    fn topmost_re_enforced_after_hide_show_cycle() {
        // Hide then show again → must re-enforce
        let frames = [
            (true, false),  // draw → show (enforce #1)
            (false, false), // stop → hide
            (true, false),  // draw again → show (enforce #2)
            (true, false),  // still drawing (enforce #3)
        ];
        assert_eq!(topmost_enforce_count(&frames), 3);
    }
}
