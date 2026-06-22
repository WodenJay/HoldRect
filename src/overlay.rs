// Transparent overlay window + softbuffer rendering

use std::io::Write;
use std::num::NonZeroU32;
use std::sync::mpsc::Receiver;

/// Diagnostic log — writes to %TEMP%\holdrect.log
fn diag(msg: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(std::env::temp_dir().join("holdrect.log"))
    {
        let _ = writeln!(f, "[{}] {}", std::process::id(), msg);
    }
}

use softbuffer::{Context, Surface};
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

/// Border color: bright red in softbuffer pixel format (0xAARRGGBB on Windows)
const BORDER_COLOR: u32 = 0xFFFF0000; // A=0xFF, R=0xFF, G=0x00, B=0x00
const BORDER_WIDTH: i32 = 4;
const TRANSPARENT: u32 = 0x00000000;

pub struct App {
    // Declaration order matters for drop order: surface is dropped first,
    // then context, then window — matching the borrow graph.
    surface: Option<Surface<&'static Window, &'static Window>>,
    context: Option<Context<&'static Window>>,
    window: Option<Window>,
    state: AppState,
    input_rx: Receiver<InputEvent>,
}

impl App {
    pub fn new(input_rx: Receiver<InputEvent>) -> Self {
        Self {
            surface: None,
            context: None,
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
            .with_transparent(true)
            .with_decorations(false)
            .with_visible(false) // start hidden
            .with_skip_taskbar(true)
            .with_position(winit::dpi::PhysicalPosition::new(position.x, position.y))
            .with_inner_size(winit::dpi::PhysicalSize::new(size.width, size.height));
        let window = event_loop.create_window(attrs).expect("Failed to create window");
        diag(&format!("window created, inner_size: {:?}", window.inner_size()));

        // Set WS_EX_TRANSPARENT for mouse passthrough
        #[cfg(windows)]
        set_click_through(&window);

        // Create context and surface inline so nothing is dropped prematurely.
        // SAFETY: We transmute the borrow lifetimes to 'static. This is sound because
        // all three (surface, context, window) are stored in the same struct, and the
        // struct's drop order (surface first, then context, then window) matches the
        // borrow graph: surface borrows context, surface borrows window.
        let context = Context::new(&window).expect("Failed to create softbuffer context");
        let surface =
            Surface::new(&context, &window).expect("Failed to create softbuffer surface");

        self.surface = Some(unsafe { std::mem::transmute(surface) });
        self.context = Some(unsafe { std::mem::transmute(context) });
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
            let old_state = format!("{:?}", self.state.drawing);
            let new_state = process_event(&self.state, &event);
            self.state = new_state;
            diag(&format!("event {:?} => {} -> {:?}", event, old_state, self.state.drawing));
        }

        // Control visibility and rendering based on state
        match &self.state.drawing {
            DrawingState::Drawing { start, current } => {
                diag(&format!("Drawing state: start={:?} current={:?}", start, current));
                if let Some(window) = &self.window {
                    let sz = window.inner_size();
                    diag(&format!("window inner_size: {}x{}", sz.width, sz.height));
                    window.set_visible(true);
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
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            diag("render: no window or surface");
            return;
        };
        let DrawingState::Drawing { start, current } = &self.state.drawing else {
            diag("render: not in Drawing state");
            return;
        };

        let size = window.inner_size();
        let (w, h) = (size.width, size.height);
        diag(&format!("render: drawing rect {:?}->{:?} on {}x{}", start, current, w, h));
        surface
            .resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap())
            .unwrap();

        let mut buffer = surface.buffer_mut().unwrap();
        // Clear to transparent
        buffer.fill(TRANSPARENT);

        let (x0, y0, x1, y1) = normalize_rect(*start, *current);

        // Draw rectangle border (4px wide)
        for y in y0..=y1 {
            for x in x0..=x1 {
                if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                    continue;
                }
                let is_border = (y - y0 < BORDER_WIDTH)
                    || (y1 - y < BORDER_WIDTH)
                    || (x - x0 < BORDER_WIDTH)
                    || (x1 - x < BORDER_WIDTH);
                if is_border {
                    buffer[(y as u32 * w + x as u32) as usize] = BORDER_COLOR;
                }
            }
        }

        buffer.present().unwrap();
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
            | WS_EX_TOPMOST.0 as isize,
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
    use windows::Win32::UI::WindowsAndMessaging::*;

    let hwnd = get_hwnd(window);
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
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
