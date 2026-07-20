use crate::cli::{
    decode_request, decode_response, encode_request, encode_response, CliCommand, CommandEnvelope,
    CommandError, MAX_WIRE_BYTES,
};
use std::sync::mpsc::Sender;
use std::time::Instant;
use windows::core::{Error as WindowsError, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
    GENERIC_READ, GENERIC_WRITE, HANDLE, WIN32_ERROR,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_MODE,
    OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use winit::event_loop::EventLoopProxy;

pub const PIPE_NAME: &str = r"\\.\pipe\HoldRect";

struct OwnedHandle(HANDLE);

// ponytail: HANDLE is a raw pointer, but Windows kernel handles are safe to transfer
// between threads; they are not tied to any thread affinity.
unsafe impl Send for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn io_error(code: &str, message: impl Into<String>) -> CommandError {
    CommandError::new(code, message)
}

fn same_win32_error(error: &WindowsError, code: windows::Win32::Foundation::WIN32_ERROR) -> bool {
    error.code() == HRESULT::from_win32(code.0)
}

fn write_all(handle: HANDLE, mut bytes: &[u8]) -> Result<(), CommandError> {
    while !bytes.is_empty() {
        let mut written = 0;
        unsafe {
            WriteFile(handle, Some(bytes), Some(&mut written), None)
                .map_err(|error| io_error("pipe_write", error.to_string()))?;
        }
        if written == 0 {
            return Err(io_error("pipe_write", "named pipe wrote zero bytes"));
        }
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

fn read_line(handle: HANDLE) -> Result<Vec<u8>, CommandError> {
    let mut frame = Vec::with_capacity(64);
    let mut chunk = [0u8; 64];
    while frame.len() < MAX_WIRE_BYTES {
        let remaining = MAX_WIRE_BYTES - frame.len();
        let capacity = remaining.min(chunk.len());
        let mut read = 0;
        unsafe {
            ReadFile(handle, Some(&mut chunk[..capacity]), Some(&mut read), None)
                .map_err(|error| io_error("pipe_read", error.to_string()))?;
        }
        if read == 0 {
            return Err(io_error("pipe_read", "named pipe closed before newline"));
        }
        frame.extend_from_slice(&chunk[..read as usize]);
        if frame.contains(&b'\n') {
            return Ok(frame);
        }
    }
    Err(io_error(
        "request_too_large",
        "wire frame exceeds 512 bytes",
    ))
}

fn create_server_pipe(pipe_name: &str) -> Result<OwnedHandle, CommandError> {
    let name = wide(pipe_name);
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            MAX_WIRE_BYTES as u32,
            MAX_WIRE_BYTES as u32,
            0,
            None,
        )
    };
    if handle.is_invalid() {
        Err(io_error(
            "pipe_create",
            WindowsError::from_win32().to_string(),
        ))
    } else {
        Ok(OwnedHandle(handle))
    }
}

fn serve_connected_pipe(
    pipe: OwnedHandle,
    command_tx: Sender<CommandEnvelope>,
    wake: impl FnOnce(),
) -> Result<(), CommandError> {
    match unsafe { ConnectNamedPipe(pipe.0, None) } {
        Ok(()) => {}
        Err(error)
            if same_win32_error(&error, ERROR_PIPE_CONNECTED)
            // ERROR_PIPE_CLOSING (232): client connected and disconnected before
            // ConnectNamedPipe completed; treat as connected, the subsequent
            // read will detect the broken pipe.
            || same_win32_error(&error, WIN32_ERROR(232u32)) => {}
        Err(error) => return Err(io_error("pipe_connect", error.to_string())),
    }

    let request = read_line(pipe.0);
    let (result, request_was_err) = match request {
        Ok(frame) => {
            let decoded = decode_request(&frame);
            let cmd_result = match decoded {
                Ok(command) => {
                    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
                    command_tx
                        .send(CommandEnvelope { command, reply_tx })
                        .map_err(|_| io_error("resident_stopped", "resident event loop stopped"))?;
                    wake();
                    reply_rx.recv().map_err(|_| {
                        io_error("resident_stopped", "resident reply channel closed")
                    })?
                }
                Err(error) => Err(error),
            };
            (cmd_result, false)
        }
        Err(error) => (Err(error), true),
    };

    match write_all(pipe.0, encode_response(&result).as_bytes()) {
        Ok(()) => {}
        Err(write_err) => {
            // If the request read also failed, prefer the original read error
            // (e.g., client disconnected mid-send). Otherwise propagate the write error.
            if request_was_err {
                return Err(result.unwrap_err());
            }
            return Err(write_err);
        }
    }
    unsafe {
        let _ = FlushFileBuffers(pipe.0);
        let _ = DisconnectNamedPipe(pipe.0);
    }
    Ok(())
}

pub fn start_server(
    pipe_name: String,
    command_tx: Sender<CommandEnvelope>,
    proxy: EventLoopProxy<()>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || loop {
        let pipe = match create_server_pipe(&pipe_name) {
            Ok(pipe) => pipe,
            Err(error) => {
                eprintln!("HoldRect IPC: {error}");
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
        };
        // ponytail: one serialized client; add per-client workers only if measured contention requires it.
        if let Err(error) = serve_connected_pipe(pipe, command_tx.clone(), || {
            let _ = proxy.send_event(());
        }) {
            eprintln!("HoldRect IPC: {error}");
        }
    })
}

fn remaining_millis(deadline: Instant) -> Result<u32, CommandError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(io_error("pipe_timeout", "timed out waiting for HoldRect"));
    }
    Ok(remaining.as_millis().clamp(1, u32::MAX as u128) as u32)
}

fn open_client(pipe_name: &str, deadline: Instant) -> Result<OwnedHandle, CommandError> {
    let name = wide(pipe_name);
    let mut saw_busy = false;
    loop {
        let result = unsafe {
            CreateFileW(
                PCWSTR(name.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            )
        };
        match result {
            Ok(handle) => return Ok(OwnedHandle(handle)),
            Err(error) if same_win32_error(&error, ERROR_FILE_NOT_FOUND) => {
                if saw_busy {
                    // Transient server-instance recreation; bounded retry.
                    let _ = remaining_millis(deadline)?;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                return Err(io_error(
                    "pipe_not_found",
                    "HoldRect command pipe is not ready",
                ));
            }
            Err(error) if same_win32_error(&error, ERROR_PIPE_BUSY) => {
                saw_busy = true;
                let wait_ms = remaining_millis(deadline)?;
                unsafe {
                    let available = WaitNamedPipeW(PCWSTR(name.as_ptr()), wait_ms);
                    if !available.as_bool() {
                        let gle = GetLastError();
                        if gle == ERROR_FILE_NOT_FOUND {
                            // Server is between instances; retry.
                            continue;
                        }
                        return Err(io_error("pipe_timeout", "timed out waiting for busy pipe"));
                    }
                }
            }
            Err(error) => return Err(io_error("pipe_open", error.to_string())),
        }
    }
}

pub fn send_command(
    pipe_name: &str,
    command: &CliCommand,
    deadline: Instant,
) -> Result<(), CommandError> {
    let pipe = open_client(pipe_name, deadline)?;
    write_all(pipe.0, encode_request(command).as_bytes())?;
    let response = read_line(pipe.0)?;
    decode_response(&response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Barrier};
    use std::time::{Duration, Instant};

    static PIPE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_pipe() -> String {
        format!(
            r"\\.\pipe\HoldRect-Test-{}-{}",
            std::process::id(),
            PIPE_COUNTER.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[test]
    fn named_pipe_round_trip_waits_for_application_reply() {
        let pipe_name = unique_pipe();
        let pipe = create_server_pipe(&pipe_name).unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let server = std::thread::spawn(move || serve_connected_pipe(pipe, command_tx, || {}));
        let app = std::thread::spawn(move || {
            let envelope = command_rx.recv().unwrap();
            assert_eq!(envelope.command, CliCommand::Clear);
            envelope.reply_tx.send(Ok(())).unwrap();
        });

        let result = send_command(
            &pipe_name,
            &CliCommand::Clear,
            Instant::now() + Duration::from_secs(2),
        );

        assert_eq!(result, Ok(()));
        app.join().unwrap();
        server.join().unwrap().unwrap();
    }

    #[test]
    fn named_pipe_propagates_application_error() {
        let pipe_name = unique_pipe();
        let pipe = create_server_pipe(&pipe_name).unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let server = std::thread::spawn(move || serve_connected_pipe(pipe, command_tx, || {}));
        let app = std::thread::spawn(move || {
            let envelope = command_rx.recv().unwrap();
            envelope
                .reply_tx
                .send(Err(CommandError::new("outside_desktop", "invalid point")))
                .unwrap();
        });

        let error = send_command(
            &pipe_name,
            &CliCommand::Magnifier {
                x: 10,
                y: 20,
                zoom: 2.0,
            },
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap_err();

        assert!(error.is_code("outside_desktop"));
        app.join().unwrap();
        server.join().unwrap().unwrap();
    }

    #[test]
    fn absent_pipe_returns_distinct_error() {
        let error = send_command(
            &unique_pipe(),
            &CliCommand::Clear,
            Instant::now() + Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(error.is_code("pipe_not_found"));
    }

    fn run_split_request_server(server_pipe: OwnedHandle, ready: Arc<Barrier>) {
        let _ = ready.wait();
        match unsafe { ConnectNamedPipe(server_pipe.0, None) } {
            Ok(()) => {}
            Err(error)
                if same_win32_error(&error, ERROR_PIPE_CONNECTED)
                // ERROR_PIPE_CLOSING (232): client connected and disconnected before
                // ConnectNamedPipe completed; treat as connected.
                || same_win32_error(&error, WIN32_ERROR(232u32)) => {}
            Err(error) => panic!("{error}"),
        }
        let frame = read_line(server_pipe.0).unwrap();
        assert_eq!(frame, b"clear\n");
    }

    #[test]
    fn request_split_across_writes_is_reassembled() {
        let pipe_name = unique_pipe();
        let server_pipe = create_server_pipe(&pipe_name).unwrap();
        let ready = Arc::new(Barrier::new(2));
        let server = {
            let ready = ready.clone();
            std::thread::spawn(move || run_split_request_server(server_pipe, ready))
        };
        ready.wait();

        let client = open_client(&pipe_name, Instant::now() + Duration::from_secs(2)).unwrap();
        write_all(client.0, b"cl").unwrap();
        write_all(client.0, b"ear\n").unwrap();
        drop(client);
        server.join().unwrap();
    }

    #[test]
    fn oversized_request_gets_error_and_next_server_instance_still_works() {
        let pipe_name = unique_pipe();
        let (command_tx, command_rx) = mpsc::channel();

        let first_pipe = create_server_pipe(&pipe_name).unwrap();
        let first_server = std::thread::spawn({
            let command_tx = command_tx.clone();
            move || serve_connected_pipe(first_pipe, command_tx, || {})
        });
        let first_client =
            open_client(&pipe_name, Instant::now() + Duration::from_secs(2)).unwrap();
        write_all(first_client.0, &vec![b'x'; MAX_WIRE_BYTES]).unwrap();
        write_all(first_client.0, b"\n").unwrap();
        let response = read_line(first_client.0).unwrap();
        assert!(decode_response(&response)
            .unwrap_err()
            .is_code("request_too_large"));
        first_server.join().unwrap().unwrap();

        let second_pipe = create_server_pipe(&pipe_name).unwrap();
        let second_server =
            std::thread::spawn(move || serve_connected_pipe(second_pipe, command_tx, || {}));
        let app = std::thread::spawn(move || {
            command_rx.recv().unwrap().reply_tx.send(Ok(())).unwrap();
        });
        assert_eq!(
            send_command(
                &pipe_name,
                &CliCommand::Clear,
                Instant::now() + Duration::from_secs(2),
            ),
            Ok(())
        );
        app.join().unwrap();
        second_server.join().unwrap().unwrap();
    }

    #[test]
    fn concurrent_clients_are_serialized_and_each_receives_a_reply() {
        let pipe_name = unique_pipe();
        let first_pipe = create_server_pipe(&pipe_name).unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let server_name = pipe_name.clone();
        let server = std::thread::spawn(move || {
            serve_connected_pipe(first_pipe, command_tx.clone(), || {}).unwrap();
            let second_pipe = create_server_pipe(&server_name).unwrap();
            serve_connected_pipe(second_pipe, command_tx, || {}).unwrap();
        });
        let app = std::thread::spawn(move || {
            for _ in 0..2 {
                command_rx.recv().unwrap().reply_tx.send(Ok(())).unwrap();
            }
        });

        let barrier = Arc::new(Barrier::new(3));
        let first = {
            let barrier = barrier.clone();
            let pipe_name = pipe_name.clone();
            std::thread::spawn(move || {
                barrier.wait();
                send_command(
                    &pipe_name,
                    &CliCommand::Clear,
                    Instant::now() + Duration::from_secs(2),
                )
            })
        };
        let second = {
            let barrier = barrier.clone();
            let pipe_name = pipe_name.clone();
            std::thread::spawn(move || {
                barrier.wait();
                send_command(
                    &pipe_name,
                    &CliCommand::Rect {
                        x1: 0,
                        y1: 0,
                        x2: 10,
                        y2: 10,
                    },
                    Instant::now() + Duration::from_secs(2),
                )
            })
        };
        barrier.wait();

        assert_eq!(first.join().unwrap(), Ok(()));
        assert_eq!(second.join().unwrap(), Ok(()));
        app.join().unwrap();
        server.join().unwrap();
    }

    #[test]
    fn client_disconnect_during_request_returns_without_panicking() {
        let pipe_name = unique_pipe();
        let pipe = create_server_pipe(&pipe_name).unwrap();
        let (command_tx, _command_rx) = mpsc::channel();
        let server = std::thread::spawn(move || serve_connected_pipe(pipe, command_tx, || {}));

        let client = open_client(&pipe_name, Instant::now() + Duration::from_secs(2)).unwrap();
        write_all(client.0, b"cl").unwrap();
        drop(client);

        let error = server.join().unwrap().unwrap_err();
        assert!(error.is_code("pipe_read"));
    }

    #[test]
    fn absent_pipe_returns_immediately_without_retry() {
        let start = Instant::now();
        let error = send_command(
            &unique_pipe(),
            &CliCommand::Clear,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(error.is_code("pipe_not_found"));
        // Must return immediately, not retry on ERROR_FILE_NOT_FOUND.
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "absent pipe retried for {}ms instead of returning immediately",
            start.elapsed().as_millis()
        );
    }

    #[test]
    fn client_disconnect_before_response_does_not_panic_server() {
        let pipe_name = unique_pipe();
        let pipe = create_server_pipe(&pipe_name).unwrap();
        let (command_tx, command_rx) = mpsc::channel();
        let server = std::thread::spawn(move || serve_connected_pipe(pipe, command_tx, || {}));
        let app = std::thread::spawn(move || {
            command_rx.recv().unwrap().reply_tx.send(Ok(())).unwrap();
        });

        let client = open_client(&pipe_name, Instant::now() + Duration::from_secs(2)).unwrap();
        write_all(client.0, b"clear\n").unwrap();
        drop(client);

        app.join().unwrap();
        let _connection_result = server.join().unwrap();
    }
}
