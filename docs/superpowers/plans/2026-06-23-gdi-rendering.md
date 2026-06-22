# GDI Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace softbuffer with Win32 GDI rendering + color-key transparency so the overlay works above all apps (including GPU-accelerated browsers and terminals).

**Architecture:** Remove softbuffer dependency. Draw rectangle borders using GDI `CreatePen` + `Rectangle`. Use `SetLayeredWindowAttributes(LWA_COLORKEY)` with magenta as the transparent color instead of winit's `with_transparent(true)` which uses DWM blur and conflicts with DirectComposition.

**Tech Stack:** Win32 GDI (`windows` crate), winit 0.30 (window management only)

## Global Constraints

- `cargo build` / `cargo test` max concurrency = 1
- No `rust-analyzer`, `cargo watch`, `clippy --watch`
- Prefer minimal `cargo test` scope; full test only for final verification
- No mockup/dead code
- Commit after each task
- TDD: tests first, then implementation

---

### Task 1: Update Cargo.toml dependencies

**Files:**
- Modify: `Cargo.toml:9-18`

**Interfaces:**
- Produces: `windows` crate with `Win32_Graphics_Gdi` feature enabled; `softbuffer` removed

- [ ] **Step 1: Edit Cargo.toml**

Remove `softbuffer = "0.4"` line. Add `"Win32_Graphics_Gdi"` to the `windows` features list.

```toml
[dependencies]
winit = "0.30"

tray-icon = "0.14"
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Foundation",
    "Win32_UI_HiDpi",
    "Win32_Graphics_Gdi",
] }
```

- [ ] **Step 2: Verify it compiles (will fail — overlay.rs still uses softbuffer)**

Run: `cargo check 2>&1 | head -5`
Expected: errors about `softbuffer` not found in `overlay.rs`

This is expected. We fix it in Task 2.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: replace softbuffer with Win32_Graphics_Gdi feature"
```

---

### Task 2: Replace softbuffer with GDI rendering in overlay.rs

This is the main task. It must be done atomically because removing softbuffer breaks compilation until GDI code replaces it.

**Files:**
- Modify: `src/overlay.rs` (full rewrite of rendering logic)

**Interfaces:**
- Consumes: `AppState`, `DrawingState`, `InputEvent`, `process_event` from `state.rs` (unchanged)
- Consumes: `get_hwnd()`, `set_click_through()`, `show_window()`, `hide_from_alt_tab()` (unchanged)
- Produces: `set_layered_color_key()` function; GDI-based `render()` method

- [ ] **Step 1: Add normalize_rect edge-case tests (TDD RED)**

Add these tests to `overlay.rs` at the bottom of the file, before the closing of the module. These test the existing `normalize_rect` function with edge cases we need to handle:

```rust
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
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib overlay::tests 2>&1`
Expected: 4 tests PASS (normalize_rect already exists and works)

- [ ] **Step 3: Rewrite overlay.rs — remove softbuffer, add GDI**

Replace the entire `overlay.rs` with the following content. Key changes:
1. Remove all `softbuffer` imports and types
2. Remove `Context`, `Surface` from `App` struct
3. Remove `.with_transparent(true)` from window creation
4. Add `set_layered_color_key()` after `set_click_through()`
5. Replace `render()` with GDI `GetDC` + `CreatePen` + `Rectangle`
6. Add `BORDER_COLOR_GDI` constant for the GDI color format

```rust
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
    use windows::Win32::Graphics::Gdi::*;
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
    use windows::Win32::Foundation::RECT;
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

        let hdc = GetDC(Some(hwnd));

        // Fill entire window with color key (transparent background)
        let full = RECT { left: 0, top: 0, right: wr.right - wr.left, bottom: wr.bottom - wr.top };
        let key_brush = CreateSolidBrush(COLORREF(COLOR_KEY));
        let _ = FillRect(hdc, &full, key_brush);
        let _ = DeleteObject(key_brush.into());

        // Draw border: red pen, null brush (no interior fill)
        let pen = CreatePen(PS_SOLID, BORDER_WIDTH, COLORREF(BORDER_COLOR_GDI));
        let old_pen = SelectObject(hdc, pen.into());
        let null_brush = GetStockObject(NULL_BRUSH);
        let old_brush = SelectObject(hdc, null_brush);

        let _ = Rectangle(hdc, x0, y0, x1, y1);

        // Restore and cleanup
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(pen.into());
        let _ = ReleaseDC(Some(hwnd), hdc);
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1`
Expected: All 39 tests pass (35 existing + 4 new normalize_rect edge cases)

- [ ] **Step 5: Build release**

Run: `cargo build --release 2>&1`
Expected: `Finished release [optimized]`

- [ ] **Step 6: Commit**

```bash
git add src/overlay.rs
git commit -m "feat: replace softbuffer with GDI rendering

- Remove softbuffer dependency, use Win32 GDI directly
- Remove with_transparent(true) to avoid DWM blur conflicts
- Use SetLayeredWindowAttributes(LWA_COLORKEY) for transparency
- GDI CreatePen + Rectangle for border drawing
- Add WS_EX_TOOLWINDOW to prevent Alt+Tab appearance
- Add normalize_rect edge-case tests"
```

---

### Task 3: Manual verification

No code changes. Manual testing only.

- [ ] **Step 1: Run the binary**

Run: `cargo run --release`

- [ ] **Step 2: Test in File Explorer**

1. Open a folder window
2. Press and hold Alt
3. Left-click and drag
4. Verify: red 4px border rectangle appears
5. Release mouse → border disappears
6. Release Alt → back to idle

- [ ] **Step 3: Test in Chrome/Edge**

1. Open Chrome or Edge
2. Navigate to any webpage
3. Press and hold Alt
4. Left-click and drag
5. Verify: red border rectangle appears ABOVE the browser

- [ ] **Step 4: Test in Windows Terminal**

1. Open Windows Terminal
2. Press and hold Alt
3. Left-click and drag
4. Verify: red border rectangle appears ABOVE the terminal

- [ ] **Step 5: Test click-through**

1. While HoldRect is idle (not drawing), click on a browser link
2. Verify: click passes through to the browser (overlay is click-through)

- [ ] **Step 6: Commit if any fixes needed**

If manual testing reveals issues, fix and commit.
