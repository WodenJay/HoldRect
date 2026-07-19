#![windows_subsystem = "windows"]

mod cli;
#[cfg(windows)]
mod autostart;
mod config;
#[cfg(windows)]
mod hook;
mod magnifier;
mod mem_report;
mod overlay;
mod popup;
#[cfg(windows)]
mod single_instance;
mod state;
mod tray;

use std::sync::mpsc;
use std::thread;

use crate::overlay::{create_event_loop, run_overlay};
use crate::state::InputEvent;
use crate::tray::{start_tray, AppExit};

fn main() {
    // --mem-report: print memory stats and exit (before GUI init)
    if std::env::args().any(|a| a == "--mem-report") {
        // Attach to parent console for stdout (we're windows_subsystem = "windows")
        unsafe {
            let _ = windows::Win32::System::Console::AttachConsole(
                windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
        crate::mem_report::print_mem_report();
        return;
    }

    // Single instance check - must be before any GUI initialization
    // Keep mutex handle alive to prevent premature release
    #[cfg(windows)]
    let _mutex_handle: Option<windows::Win32::Foundation::HANDLE> = match crate::single_instance::try_acquire() {
        Ok(crate::single_instance::SingleInstance::First(handle)) => Some(handle),
        Ok(crate::single_instance::SingleInstance::AlreadyRunning) => {
            crate::single_instance::notify_existing_instance();
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("HoldRect: single-instance check failed: {e}, continuing anyway");
            None
        }
    };

    #[cfg(windows)]
    set_dpi_awareness();

    let (event_loop, proxy) = create_event_loop();
    let (input_tx, input_rx) = mpsc::channel::<InputEvent>();
    let (exit_tx, exit_rx) = mpsc::channel::<AppExit>();
    let (config_tx, config_rx) = mpsc::channel::<crate::config::AppConfig>();

    // Start Win32 input hook listener (replaces rdev)
    let config = crate::config::AppConfig::load();
    let hook_tx = input_tx.clone();
    #[cfg(windows)]
    crate::hook::start_hook_listener(hook_tx, proxy, config.modifier_vk_codes);

    // Send FirstLaunch event for welcome popup
    let _ = input_tx.send(crate::state::InputEvent::FirstLaunch);

    // Spawn config file watcher thread for hot-reload
    let watch_dir = dirs::home_dir()
        .map(|h| h.join(".holdrect"))
        .unwrap_or_default();
    thread::spawn(move || {
        crate::config::watch_config_dir(watch_dir, config_tx);
    });

    let _tray_icon = start_tray(exit_tx);

    thread::spawn(move || {
        let _ = exit_rx.recv();
        std::process::exit(0);
    });

    run_overlay(
        event_loop,
        input_rx,
        config_rx,
        config.border_width,
        config.color_mode,
        config.modifier_name.clone(),
    );
    std::process::exit(0);
}

#[cfg(windows)]
fn set_dpi_awareness() {
    use windows::Win32::UI::HiDpi::*;
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
