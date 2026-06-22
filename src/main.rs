mod state;
mod input;
mod overlay;
mod tray;

use std::sync::mpsc;
use std::thread;

use crate::input::start_input_listener;
use crate::overlay::run_overlay;
use crate::state::InputEvent;
use crate::tray::{start_tray, AppExit};

fn main() {
    // Set DPI awareness before any window creation
    #[cfg(windows)]
    set_dpi_awareness();

    // Channel: rdev input -> main event loop
    let (input_tx, input_rx) = mpsc::channel::<InputEvent>();

    // Channel: tray exit -> main
    let (exit_tx, exit_rx) = mpsc::channel::<AppExit>();

    // Start rdev input listener in background thread
    thread::spawn(move || {
        start_input_listener(input_tx);
    });

    // Start system tray (keeps TrayIcon alive)
    let _tray_icon = start_tray(exit_tx);

    // Monitor for exit signal in background
    thread::spawn(move || {
        let _ = exit_rx.recv();
        std::process::exit(0);
    });

    // Run overlay on main thread (winit requires main thread)
    run_overlay(input_rx);

    // If overlay exits normally (window closed), terminate the process.
    // The exit_rx thread only handles tray-initiated quit.
    std::process::exit(0);
}

#[cfg(windows)]
fn set_dpi_awareness() {
    use windows::Win32::UI::HiDpi::*;
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_SYSTEM_AWARE);
    }
}
