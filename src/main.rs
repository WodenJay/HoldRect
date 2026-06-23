#![windows_subsystem = "windows"]

mod state;
mod overlay;
mod tray;
#[cfg(windows)]
mod hook;

use std::sync::mpsc;
use std::thread;

use crate::overlay::{create_event_loop, run_overlay};
use crate::state::InputEvent;
use crate::tray::{start_tray, AppExit};

fn main() {
    #[cfg(windows)]
    set_dpi_awareness();

    let (event_loop, proxy) = create_event_loop();
    let (input_tx, input_rx) = mpsc::channel::<InputEvent>();
    let (exit_tx, exit_rx) = mpsc::channel::<AppExit>();

    // Start Win32 input hook listener (replaces rdev)
    #[cfg(windows)]
    crate::hook::start_hook_listener(input_tx, proxy);

    let _tray_icon = start_tray(exit_tx);

    thread::spawn(move || {
        let _ = exit_rx.recv();
        std::process::exit(0);
    });

    run_overlay(event_loop, input_rx);
    std::process::exit(0);
}

#[cfg(windows)]
fn set_dpi_awareness() {
    use windows::Win32::UI::HiDpi::*;
    unsafe {
        // Per-monitor V2: mouse physical coords match overlay coords across all monitors
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
