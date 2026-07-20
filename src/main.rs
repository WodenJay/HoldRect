#![windows_subsystem = "windows"]

mod cli;
#[cfg(windows)]
mod autostart;
#[cfg(windows)]
mod ipc;
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
use std::time::{Duration, Instant};

use crate::cli::{CliCommand, CommandError, StartupMode};
use crate::overlay::{create_event_loop, run_overlay};
use crate::state::InputEvent;
use crate::tray::{start_tray, AppExit};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const STARTUP_RETRY_DELAY: Duration = Duration::from_millis(25);

fn deliver_with_auto_start<Send, Spawn>(
    command: &CliCommand,
    deadline: Instant,
    retry_delay: Duration,
    mut send: Send,
    spawn: Spawn,
) -> Result<(), CommandError>
where
    Send: FnMut(&CliCommand, Instant) -> Result<(), CommandError>,
    Spawn: FnOnce() -> Result<(), CommandError>,
{
    match send(command, deadline) {
        Ok(()) => return Ok(()),
        Err(error) if !error.is_code("pipe_not_found") => return Err(error),
        Err(_) => spawn()?,
    }

    loop {
        if Instant::now() >= deadline {
            return Err(CommandError::new(
                "pipe_timeout",
                "timed out waiting for HoldRect to start",
            ));
        }
        match send(command, deadline) {
            Ok(()) => return Ok(()),
            Err(error) if error.is_code("pipe_not_found") => {
                std::thread::sleep(retry_delay);
            }
            Err(error) => return Err(error),
        }
    }
}

fn spawn_daemon() -> Result<(), CommandError> {
    let executable = std::env::current_exe()
        .map_err(|error| CommandError::new("spawn_failed", error.to_string()))?;
    std::process::Command::new(executable)
        .arg("--daemon")
        .spawn()
        .map_err(|error| CommandError::new("spawn_failed", error.to_string()))?;
    Ok(())
}

fn run_client(command: &CliCommand) -> Result<(), CommandError> {
    deliver_with_auto_start(
        command,
        Instant::now() + STARTUP_TIMEOUT,
        STARTUP_RETRY_DELAY,
        |command, deadline| crate::ipc::send_command(crate::ipc::PIPE_NAME, command, deadline),
        spawn_daemon,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match crate::cli::parse_startup_args(&args) {
        Ok(mode) => mode,
        Err(error) => {
            attach_parent_console();
            eprintln!("HoldRect: {error}");
            std::process::exit(2);
        }
    };

    match mode {
        StartupMode::MemoryReport => {
            attach_parent_console();
            crate::mem_report::print_mem_report();
        }
        StartupMode::Client(command) => {
            attach_parent_console();
            match run_client(&command) {
                Ok(()) => println!("OK"),
                Err(error) => {
                    eprintln!("HoldRect: {error}");
                    std::process::exit(1);
                }
            }
        }
        StartupMode::Resident { first_launch } => run_resident(first_launch),
    }
}

fn attach_parent_console() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Console::AttachConsole(
            windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
        );
    }
}

fn run_resident(first_launch: bool) {
    #[cfg(windows)]
    let _mutex_handle: Option<windows::Win32::Foundation::HANDLE> =
        match crate::single_instance::try_acquire() {
            Ok(crate::single_instance::SingleInstance::First(handle)) => Some(handle),
            Ok(crate::single_instance::SingleInstance::AlreadyRunning) => {
                if first_launch {
                    crate::single_instance::notify_existing_instance();
                }
                return;
            }
            Err(error) => {
                eprintln!("HoldRect: single-instance check failed: {error}, continuing anyway");
                None
            }
        };

    #[cfg(windows)]
    set_dpi_awareness();

    let (event_loop, proxy) = create_event_loop();
    let (input_tx, input_rx) = mpsc::channel::<InputEvent>();
    let (command_tx, command_rx) = mpsc::channel::<crate::cli::CommandEnvelope>();
    let (exit_tx, exit_rx) = mpsc::channel::<AppExit>();
    let (config_tx, config_rx) = mpsc::channel::<crate::config::AppConfig>();

    let config = crate::config::AppConfig::load();
    #[cfg(windows)]
    crate::hook::start_hook_listener(
        input_tx.clone(),
        proxy.clone(),
        config.modifier_vk_codes.clone(),
    );
    #[cfg(windows)]
    let _ipc_thread = crate::ipc::start_server(
        crate::ipc::PIPE_NAME.to_owned(),
        command_tx,
        proxy,
    );

    if first_launch {
        let _ = input_tx.send(InputEvent::FirstLaunch);
    }

    let watch_dir = dirs::home_dir()
        .map(|home| home.join(".holdrect"))
        .unwrap_or_default();
    thread::spawn(move || crate::config::watch_config_dir(watch_dir, config_tx));

    let _tray_icon = start_tray(exit_tx);
    thread::spawn(move || {
        let _ = exit_rx.recv();
        std::process::exit(0);
    });

    run_overlay(
        event_loop,
        input_rx,
        config_rx,
        command_rx,
        config.border_width,
        config.color_mode,
        config.modifier_name,
    );
}

#[cfg(windows)]
fn set_dpi_awareness() {
    use windows::Win32::UI::HiDpi::*;
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    #[test]
    fn ready_daemon_does_not_spawn() {
        let spawned = Cell::new(0);
        let result = deliver_with_auto_start(
            &CliCommand::Clear,
            Instant::now() + Duration::from_secs(1),
            Duration::ZERO,
            |_, _| Ok(()),
            || {
                spawned.set(spawned.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result, Ok(()));
        assert_eq!(spawned.get(), 0);
    }

    #[test]
    fn missing_daemon_spawns_once_then_retries() {
        let spawned = Cell::new(0);
        let mut results = VecDeque::from([
            Err(CommandError::new("pipe_not_found", "missing")),
            Err(CommandError::new("pipe_not_found", "starting")),
            Ok(()),
        ]);
        let result = deliver_with_auto_start(
            &CliCommand::Clear,
            Instant::now() + Duration::from_secs(1),
            Duration::ZERO,
            |_, _| results.pop_front().unwrap(),
            || {
                spawned.set(spawned.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result, Ok(()));
        assert_eq!(spawned.get(), 1);
    }

    #[test]
    fn non_missing_error_does_not_spawn() {
        let spawned = Cell::new(0);
        let error = deliver_with_auto_start(
            &CliCommand::Clear,
            Instant::now() + Duration::from_secs(1),
            Duration::ZERO,
            |_, _| Err(CommandError::new("pipe_open", "denied")),
            || {
                spawned.set(spawned.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.is_code("pipe_open"));
        assert_eq!(spawned.get(), 0);
    }

    #[test]
    fn missing_daemon_respects_deadline() {
        let error = deliver_with_auto_start(
            &CliCommand::Clear,
            Instant::now(),
            Duration::ZERO,
            |_, _| Err(CommandError::new("pipe_not_found", "missing")),
            || Ok(()),
        )
        .unwrap_err();
        assert!(error.is_code("pipe_timeout"));
    }
}
