// Transparent overlay window + GDI rendering

use std::sync::mpsc::Receiver;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::raw_window_handle::HasWindowHandle;
use winit::window::{Window, WindowId};

#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;

use crate::state::AppState;
use crate::state::DrawingState;
use crate::state::InputEvent;
use crate::state::process_event;

const BORDER_WIDTH: i32 = 4;
// GDI uses COLORREF: 0x00BBGGRR
const BORDER_COLOR_GDI: u32 = 0x000000FF; // Red in GDI format
const COLOR_KEY: u32 = 0x00FF00FF; // Magenta — transparent color key

pub struct App {
    window: Option<Window>,
    state: AppState,
    input_rx: Receiver<InputEvent>,
}

impl App {
    pub fn new(input_rx: Receiver<InputEvent>) -> Self {
        Self {
            window: None,
            state: AppState::default(),
            input_rx,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let monitor = event_loop.primary_monitor().unwrap();
        let size = monitor.size();
        let position = monitor.position();

        let attrs = Window::default_attributes()
            .with_title("HoldRect")
            .with_decorations(false)
            .with_visible(false) // start hidden
            .with_skip_taskbar(true)
            .with_position(winit::dpi::PhysicalPosition::new(position.x, position.y))
            .with_inner_size(winit::dpi::PhysicalSize::new(size.width, size.height));
        let window = event_loop.create_window(attrs).expect("Failed to create window");

        // Set WS_EX_TRANSPARENT for mouse passthrough + WS_EX_LAYERED
        #[cfg(windows)]
        set_click_through(&window);

        // Set color-key transparency AFTER set_click_through (needs WS_EX_LAYERED)
        #[cfg(windows)]
        set_layered_color_key(&window);

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
            DrawingState::Drawing { .. } => {
                if let Some(window) = &self.window {
                    window.set_visible(true);
                    // SetWindowPos(HWND_TOPMOST) pins above all windows;
                    // must come AFTER set_visible so WS_VISIBLE is set.
                    #[cfg(windows)]
                    show_window(window);
                    window.request_redraw();
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
        draw_border(window, *start, *current);
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
fn set_layered_color_key(window: &Window) {
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let _ = SetLayeredWindowAttributes(
            hwnd,
            COLORREF(COLOR_KEY),
            0,
            LWA_COLORKEY,
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
fn show_window(window: &Window) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        // HWND_TOPMOST = (HWND)(LONG_PTR)-1 in Win32
        let topmost = HWND(-1isize as *mut core::ffi::c_void);
        let _ = SetWindowPos(
            hwnd,
            topmost,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Draw border rectangle using GDI.
/// Mouse coordinates are screen-absolute; convert to window-local.
#[cfg(windows)]
fn draw_border(window: &Window, start: (i32, i32), current: (i32, i32)) {
    use windows::Win32::Foundation::{COLORREF, RECT};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        // Get window position for coordinate conversion
        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);

        // Convert screen coords to window-local
        let (x0, y0, x1, y1) = normalize_rect(start, current);
        let x0 = x0 - wr.left;
        let y0 = y0 - wr.top;
        let x1 = x1 - wr.left;
        let y1 = y1 - wr.top;

        let hdc = GetDC(hwnd);

        // Fill entire window with color key (transparent background)
        let full = RECT { left: 0, top: 0, right: wr.right - wr.left, bottom: wr.bottom - wr.top };
        let key_brush = CreateSolidBrush(COLORREF(COLOR_KEY));
        let _ = FillRect(hdc, &full, key_brush);
        let _ = DeleteObject(key_brush);

        // Draw border: red pen, null brush (no interior fill)
        let pen = CreatePen(PS_SOLID, BORDER_WIDTH, COLORREF(BORDER_COLOR_GDI));
        let old_pen = SelectObject(hdc, pen);
        let null_brush = GetStockObject(NULL_BRUSH);
        let old_brush = SelectObject(hdc, null_brush);

        let _ = Rectangle(hdc, x0, y0, x1, y1);

        // Restore and cleanup
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(pen);
        let _ = ReleaseDC(hwnd, hdc);
    }
}

/// Create the overlay event loop and proxy. Caller owns the event loop.
pub fn create_event_loop() -> (EventLoop<()>, EventLoopProxy<()>) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let proxy = event_loop.create_proxy();
    (event_loop, proxy)
}

/// Run the overlay event loop on the main thread. Blocks until exit.
pub fn run_overlay(event_loop: EventLoop<()>, input_rx: Receiver<InputEvent>) {
    let mut app = App::new(input_rx);
    event_loop.run_app(&mut app).expect("Event loop error");
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
}
