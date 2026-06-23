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
                    draw_border(window, *start, *current);
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
            draw_border(window, *start, *current);
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

/// Draw border rectangle using UpdateLayeredWindow with per-pixel alpha.
/// Background is fully transparent (alpha=0), border pixels are opaque red.
#[cfg(windows)]
fn draw_border(window: &Window, start: (i32, i32), current: (i32, i32)) {
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

        let bitmap = match CreateDIBSection(
            None,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut pixels as *mut *mut u8 as _,
            None,
            0,
        ) {
            Ok(bitmap) => bitmap,
            Err(_) => return,
        };

        if pixels.is_null() {
            let _ = DeleteObject(bitmap);
            return;
        }

        let screen_dc = GetDC(HWND::default());
        let memory_dc = CreateCompatibleDC(screen_dc);
        let old_bitmap = SelectObject(memory_dc, bitmap);

        // Entire window fully transparent
        std::ptr::write_bytes(
            pixels,
            0,
            width as usize * height as usize * 4,
        );

        let (global_x0, global_y0, global_x1, global_y1) =
            normalize_rect(start, current);

        let x0 = (global_x0 - wr.left).clamp(0, width - 1);
        let y0 = (global_y0 - wr.top).clamp(0, height - 1);
        let x1 = (global_x1 - wr.left).clamp(0, width - 1);
        let y1 = (global_y1 - wr.top).clamp(0, height - 1);

        let stride = width as usize * 4;

        let set_red_pixel = |x: i32, y: i32| {
            if x < 0 || x >= width || y < 0 || y >= height {
                return;
            }

            let pixel = pixels.add(y as usize * stride + x as usize * 4);

            // BGRA — alpha must be 255
            *pixel.add(0) = 0;
            *pixel.add(1) = 0;
            *pixel.add(2) = 255;
            *pixel.add(3) = 255;
        };

        if x1 > x0 && y1 > y0 {
            for offset in 0..BORDER_WIDTH {
                let top = y0 + offset;
                let bottom = y1 - offset;
                let left = x0 + offset;
                let right = x1 - offset;

                for x in left..=right {
                    set_red_pixel(x, top);
                    set_red_pixel(x, bottom);
                }

                for y in top..=bottom {
                    set_red_pixel(left, y);
                    set_red_pixel(right, y);
                }
            }
        }

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

        let result = UpdateLayeredWindow(
            hwnd,
            screen_dc,
            Some(&destination),
            Some(&size),
            memory_dc,
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );

        if let Err(error) = result {
            eprintln!("UpdateLayeredWindow failed: {error:?}");
        }

        SelectObject(memory_dc, old_bitmap);
        let _ = DeleteObject(bitmap);
        let _ = DeleteDC(memory_dc);
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
