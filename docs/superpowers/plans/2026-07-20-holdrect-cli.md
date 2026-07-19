# HoldRect CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a single-executable Windows CLI that auto-starts the resident HoldRect process, draws persistent rectangles, shows a fixed magnifier at physical coordinates, and clears all visual state through acknowledged named-pipe commands.

**Architecture:** Keep command parsing and the text protocol pure in `src/cli.rs`, apply commands on the existing winit event-loop thread in `src/overlay.rs`, and isolate Win32 named-pipe I/O in `src/ipc.rs`. CLI clients skip the resident mutex, send one request, wait until the resident event loop applies it, and then exit; resident mode keeps all existing hook, tray, configuration, and rendering paths.

**Tech Stack:** Rust 2021, standard library `std::sync::mpsc`, winit 0.30, windows crate 0.58 (`Win32_System_Pipes` added to existing features), Win32 named pipes, inline Rust unit/integration tests.

## Global Constraints

- Windows-first implementation; do not add macOS/Linux IPC in this plan.
- Keep one distributed `holdrect.exe`; no second CLI binary.
- Add no third-party dependency and no async runtime.
- Public commands are exactly `rect`, `magnifier`, and `clear`.
- Coordinates are signed `i32` physical pixels in the Windows virtual-desktop coordinate space.
- Rectangle arguments are two corners passed as `x1 y1 x2 y2`; zero-area rectangles are invalid.
- Magnifier zoom defaults to `2.0` and must be within inclusive range `1.5..=8.0`.
- CLI rectangles persist until `clear`; no IDs, querying, individual deletion, or timed expiry.
- Success means the resident event loop applied the command; do not add rendered-frame acknowledgment.
- Preserve the existing `--mem-report` path and normal no-argument startup behavior.
- The internal `--daemon` mode suppresses only the `FirstLaunch` popup.
- Keep resident IPC idle behavior blocking rather than polling.
- Follow strict red → green TDD. Do not write production behavior before its failing test.
- Run Cargo build/test commands with one job: `-j 1`.
- Commit after every task and run a review gate before beginning the next task.
- Do not refactor unrelated parts of the large existing `overlay.rs`, `state.rs`, or `hook.rs` modules.

## Planned File Structure

- Create `src/cli.rs`: command types, startup-mode parser, value validation, request/response codec, and in-process command envelope.
- Create `src/ipc.rs`: Windows named-pipe handle ownership, client, serialized server, bounded line I/O, and Windows-only transport tests.
- Modify `src/main.rs`: parse modes before the mutex, auto-start daemon clients, wire the command channel/server, and preserve existing modes.
- Modify `src/overlay.rs`: receive command envelopes, cache virtual desktop bounds, apply commands, acknowledge them, and select fixed versus cursor magnifier coordinates.
- Modify `Cargo.toml`: enable only `Win32_System_Pipes` on the existing `windows` dependency.
- Modify `README.md`: document the three commands, coordinate space, persistence, and auto-start behavior.

---

## Milestone 1: Pure Command Contract

### Task 1: Add the command model and startup parser

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs:3-14`
- Test: `src/cli.rs` inline `#[cfg(test)]` module

**Interfaces:**
- Consumes: raw argument values from `std::env::args().skip(1)`.
- Produces: `CliCommand`, `StartupMode`, `CommandError`, `CommandEnvelope`, and `parse_startup_args(args: &[String]) -> Result<StartupMode, CommandError>`.
- Later tasks rely on `CommandError::new`, `CommandError::is_code`, and `CommandEnvelope { command, reply_tx }` exactly as defined here.

- [ ] **Step 1: Add the module declaration and failing parser tests**

Add `mod cli;` beside the existing module declarations in `src/main.rs`. Create `src/cli.rs` with this test module first; leave the imported production names undefined so the first run proves the tests are red:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn no_arguments_select_normal_resident_mode() {
        assert_eq!(
            parse_startup_args(&[]).unwrap(),
            StartupMode::Resident { first_launch: true }
        );
    }

    #[test]
    fn daemon_selects_silent_resident_mode() {
        assert_eq!(
            parse_startup_args(&args(&["--daemon"])).unwrap(),
            StartupMode::Resident { first_launch: false }
        );
    }

    #[test]
    fn mem_report_remains_immediate_mode() {
        assert_eq!(
            parse_startup_args(&args(&["--mem-report"])).unwrap(),
            StartupMode::MemoryReport
        );
    }

    #[test]
    fn rect_accepts_signed_reversed_corners() {
        assert_eq!(
            parse_startup_args(&args(&["rect", "500", "-20", "100", "400"])).unwrap(),
            StartupMode::Client(CliCommand::Rect {
                x1: 500,
                y1: -20,
                x2: 100,
                y2: 400,
            })
        );
    }

    #[test]
    fn magnifier_uses_default_zoom() {
        assert_eq!(
            parse_startup_args(&args(&["magnifier", "800", "450"])).unwrap(),
            StartupMode::Client(CliCommand::Magnifier {
                x: 800,
                y: 450,
                zoom: 2.0,
            })
        );
    }

    #[test]
    fn magnifier_accepts_zoom_boundaries() {
        for zoom in ["1.5", "8"] {
            assert!(parse_startup_args(&args(&["magnifier", "0", "0", zoom])).is_ok());
        }
    }

    #[test]
    fn clear_has_no_arguments() {
        assert_eq!(
            parse_startup_args(&args(&["clear"])).unwrap(),
            StartupMode::Client(CliCommand::Clear)
        );
    }

    #[test]
    fn invalid_commands_are_rejected() {
        let cases = [
            args(&["rect", "0", "0", "0", "10"]),
            args(&["rect", "0", "0", "10", "0"]),
            args(&["rect", "x", "0", "10", "10"]),
            args(&["rect", "0", "0", "2147483648", "10"]),
            args(&["magnifier", "0", "0", "1.49"]),
            args(&["magnifier", "0", "0", "8.01"]),
            args(&["magnifier", "0", "0", "NaN"]),
            args(&["magnifier", "0", "0", "inf"]),
            args(&["clear", "extra"]),
            args(&["unknown"]),
            args(&["--daemon", "extra"]),
            args(&["--mem-report", "extra"]),
        ];

        for case in cases {
            assert!(parse_startup_args(&case).is_err(), "accepted {case:?}");
        }
    }
}
```

- [ ] **Step 2: Run the parser tests and verify the red state**

Run:

```powershell
cargo test -j 1 cli::tests
```

Expected: compilation fails because `CliCommand`, `StartupMode`, `CommandError`, and `parse_startup_args` are not defined. A green result at this step means the tests are not exercising the missing feature and must be corrected.

- [ ] **Step 3: Implement the minimal command model and parser**

Add the following production code above the tests in `src/cli.rs`:

```rust
use std::fmt;
use std::sync::mpsc::Sender;

pub const DEFAULT_ZOOM: f64 = 2.0;
pub const MIN_ZOOM: f64 = 1.5;
pub const MAX_ZOOM: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub enum CliCommand {
    Rect {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },
    Magnifier {
        x: i32,
        y: i32,
        zoom: f64,
    },
    Clear,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StartupMode {
    Resident { first_launch: bool },
    Client(CliCommand),
    MemoryReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn is_code(&self, code: &str) -> bool {
        self.code == code
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

pub struct CommandEnvelope {
    pub command: CliCommand,
    pub reply_tx: Sender<Result<(), CommandError>>,
}

pub fn parse_startup_args(args: &[String]) -> Result<StartupMode, CommandError> {
    if args.is_empty() {
        return Ok(StartupMode::Resident { first_launch: true });
    }
    if args.len() == 1 && args[0] == "--daemon" {
        return Ok(StartupMode::Resident { first_launch: false });
    }
    if args.len() == 1 && args[0] == "--mem-report" {
        return Ok(StartupMode::MemoryReport);
    }

    parse_visual_command(args).map(StartupMode::Client)
}

fn parse_visual_command(args: &[String]) -> Result<CliCommand, CommandError> {
    match args.first().map(String::as_str) {
        Some("rect") if args.len() == 5 => {
            let x1 = parse_i32(&args[1], "x1")?;
            let y1 = parse_i32(&args[2], "y1")?;
            let x2 = parse_i32(&args[3], "x2")?;
            let y2 = parse_i32(&args[4], "y2")?;
            if x1 == x2 || y1 == y2 {
                return Err(CommandError::new(
                    "invalid_rect",
                    "rectangle width and height must be non-zero",
                ));
            }
            Ok(CliCommand::Rect { x1, y1, x2, y2 })
        }
        Some("magnifier") if args.len() == 3 || args.len() == 4 => {
            let x = parse_i32(&args[1], "x")?;
            let y = parse_i32(&args[2], "y")?;
            let zoom = if args.len() == 4 {
                args[3]
                    .parse::<f64>()
                    .map_err(|_| CommandError::new("invalid_zoom", "zoom must be a number"))?
            } else {
                DEFAULT_ZOOM
            };
            if !zoom.is_finite() || !(MIN_ZOOM..=MAX_ZOOM).contains(&zoom) {
                return Err(CommandError::new(
                    "invalid_zoom",
                    format!("zoom must be between {MIN_ZOOM} and {MAX_ZOOM}"),
                ));
            }
            Ok(CliCommand::Magnifier { x, y, zoom })
        }
        Some("clear") if args.len() == 1 => Ok(CliCommand::Clear),
        _ => Err(CommandError::new(
            "usage",
            "usage: holdrect rect x1 y1 x2 y2 | magnifier x y [zoom] | clear",
        )),
    }
}

fn parse_i32(value: &str, name: &str) -> Result<i32, CommandError> {
    value.parse::<i32>().map_err(|_| {
        CommandError::new(
            "invalid_coordinate",
            format!("{name} must be a signed 32-bit integer"),
        )
    })
}
```

- [ ] **Step 4: Run focused and existing state tests**

Run:

```powershell
cargo test -j 1 cli::tests
cargo test -j 1 state::tests
```

Expected: both commands pass. This confirms the new pure model did not alter the existing manual input state machine.

- [ ] **Step 5: Commit the command model**

```powershell
git add src/cli.rs src/main.rs
git commit -m "feat(cli): parse HoldRect commands"
```

### Task 2: Add the bounded text protocol

**Files:**
- Modify: `src/cli.rs`
- Test: `src/cli.rs` inline `#[cfg(test)]` module

**Interfaces:**
- Consumes: `CliCommand` and `CommandError` from Task 1.
- Produces: `MAX_WIRE_BYTES`, `encode_request`, `decode_request`, `encode_response`, and `decode_response` for `src/ipc.rs`.

- [ ] **Step 1: Write failing protocol tests**

Append these tests to `src/cli.rs`:

```rust
#[test]
fn request_round_trips_every_command() {
    let commands = [
        CliCommand::Rect {
            x1: -100,
            y1: 20,
            x2: 400,
            y2: 500,
        },
        CliCommand::Magnifier {
            x: 800,
            y: 450,
            zoom: 3.0,
        },
        CliCommand::Clear,
    ];

    for command in commands {
        let encoded = encode_request(&command);
        assert_eq!(decode_request(encoded.as_bytes()).unwrap(), command);
    }
}

#[test]
fn response_round_trips_ok_and_error() {
    let ok = encode_response(&Ok(()));
    assert_eq!(decode_response(ok.as_bytes()), Ok(()));

    let error = CommandError::new("invalid_rect", "outside desktop");
    let encoded = encode_response(&Err(error.clone()));
    assert_eq!(decode_response(encoded.as_bytes()), Err(error));
}

#[test]
fn wire_decoder_rejects_invalid_frames() {
    let mut oversized = vec![b'x'; MAX_WIRE_BYTES + 1];
    oversized[MAX_WIRE_BYTES] = b'\n';
    let cases: Vec<Vec<u8>> = vec![
        b"clear".to_vec(),
        b"clear\nextra\n".to_vec(),
        vec![0xff, b'\n'],
        oversized,
        b"rect 0 0 0 10\n".to_vec(),
        b"clear extra\n".to_vec(),
    ];

    for case in cases {
        assert!(decode_request(&case).is_err(), "accepted {case:?}");
    }
}

#[test]
fn response_sanitizes_newlines() {
    let encoded = encode_response(&Err(CommandError::new("bad", "line1\r\nline2")));
    assert_eq!(encoded, "ERR bad line1  line2\n");
}
```

- [ ] **Step 2: Run the protocol tests and verify failure**

Run:

```powershell
cargo test -j 1 cli::tests::request_round_trips_every_command
```

Expected: compilation fails because the protocol constants and functions are undefined.

- [ ] **Step 3: Implement the protocol without serialization dependencies**

Add this code above the tests:

```rust
pub const MAX_WIRE_BYTES: usize = 512;

pub fn encode_request(command: &CliCommand) -> String {
    match command {
        CliCommand::Rect { x1, y1, x2, y2 } => {
            format!("rect {x1} {y1} {x2} {y2}\n")
        }
        CliCommand::Magnifier { x, y, zoom } => {
            format!("magnifier {x} {y} {zoom}\n")
        }
        CliCommand::Clear => "clear\n".to_owned(),
    }
}

pub fn decode_request(bytes: &[u8]) -> Result<CliCommand, CommandError> {
    let line = decode_line(bytes)?;
    let args: Vec<String> = line
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect();
    match parse_startup_args(&args)? {
        StartupMode::Client(command) => Ok(command),
        _ => Err(CommandError::new(
            "invalid_command",
            "wire request must be a visual command",
        )),
    }
}

pub fn encode_response(result: &Result<(), CommandError>) -> String {
    match result {
        Ok(()) => "OK\n".to_owned(),
        Err(error) => {
            let message = error.message.replace('\r', " ").replace('\n', " ");
            format!("ERR {} {}\n", error.code, message)
        }
    }
}

pub fn decode_response(bytes: &[u8]) -> Result<(), CommandError> {
    let line = decode_line(bytes)?;
    if line == "OK" {
        return Ok(());
    }
    let mut parts = line.splitn(3, ' ');
    if parts.next() != Some("ERR") {
        return Err(CommandError::new("invalid_response", "unknown response"));
    }
    let code = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandError::new("invalid_response", "missing error code"))?;
    let message = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CommandError::new("invalid_response", "missing error message"))?;
    Err(CommandError::new(code, message))
}

fn decode_line(bytes: &[u8]) -> Result<&str, CommandError> {
    if bytes.len() > MAX_WIRE_BYTES {
        return Err(CommandError::new("request_too_large", "wire frame exceeds 512 bytes"));
    }
    let body = bytes
        .strip_suffix(b"\n")
        .ok_or_else(|| CommandError::new("invalid_frame", "wire frame must end with newline"))?;
    if body.contains(&b'\r') || body.contains(&b'\n') {
        return Err(CommandError::new("invalid_frame", "wire frame contains extra line break"));
    }
    std::str::from_utf8(body)
        .map_err(|_| CommandError::new("invalid_utf8", "wire frame must be UTF-8"))
}
```

- [ ] **Step 4: Run all CLI tests**

```powershell
cargo test -j 1 cli::tests
```

Expected: all Task 1 and Task 2 tests pass.

- [ ] **Step 5: Commit the protocol**

```powershell
git add src/cli.rs
git commit -m "feat(cli): encode command protocol"
```

---

## Milestone 2: Resident Command Application

### Task 3: Apply commands on the overlay event-loop thread

**Files:**
- Modify: `src/overlay.rs:19-28,188-253,258-296,395-507,962-971`
- Modify: `src/main.rs:55-58,84-93` to keep the binary compiling with the new receiver
- Test: `src/overlay.rs` inline test module near existing fade/application tests

**Interfaces:**
- Consumes: `CliCommand`, `CommandEnvelope`, and `CommandError` from Task 1.
- Produces: `apply_cli_command(state: &mut AppState, fades: &mut Vec<FadingRect>, magnifier_position: &mut Option<(i32, i32)>, desktop: (i32, i32, i32, i32), command: &CliCommand) -> Result<(), CommandError>`, `App.command_rx`, `App.magnifier_position`, `App.virtual_desktop_rect`, and `run_overlay(event_loop, input_rx, config_rx, command_rx, border_width, color_mode, modifier_name)`.
- Task 5 reuses the `Sender<CommandEnvelope>` created beside the receiver and starts the IPC server.

- [ ] **Step 1: Write failing pure command-application tests**

Import CLI types into `src/overlay.rs`, then add these tests. Reuse the existing test module's `super::*`, `Duration`, and `Instant` imports:

```rust
fn apply_command(
    state: &mut AppState,
    fades: &mut Vec<FadingRect>,
    magnifier_position: &mut Option<(i32, i32)>,
    command: &CliCommand,
) -> Result<(), CommandError> {
    apply_cli_command(
        state,
        fades,
        magnifier_position,
        (-1920, 0, 1920, 1080),
        command,
    )
}

#[test]
fn cli_rect_normalizes_and_preserves_manual_flags() {
    let mut state = AppState {
        drawing: DrawingState::Drawing {
            start: (1, 2),
            current: (3, 4),
        },
        pinned_active: true,
        spotlight_active: true,
        ..Default::default()
    };
    let original_drawing = state.drawing.clone();
    let mut fades = Vec::new();
    let mut position = None;

    apply_command(
        &mut state,
        &mut fades,
        &mut position,
        &CliCommand::Rect {
            x1: 500,
            y1: 400,
            x2: 100,
            y2: 200,
        },
    )
    .unwrap();

    assert_eq!(state.drawing, original_drawing);
    assert!(state.pinned_active);
    assert!(state.spotlight_active);
    assert_eq!(
        state.pinned_rects.last().unwrap(),
        &crate::state::PinnedRect {
            x0: 100,
            y0: 200,
            x1: 500,
            y1: 400,
            spotlight: false,
        }
    );
}

#[test]
fn cli_rects_accumulate() {
    let mut state = AppState::default();
    let mut fades = Vec::new();
    let mut position = None;
    for x in [0, 100] {
        apply_command(
            &mut state,
            &mut fades,
            &mut position,
            &CliCommand::Rect {
                x1: x,
                y1: 10,
                x2: x + 50,
                y2: 60,
            },
        )
        .unwrap();
    }
    assert_eq!(state.pinned_rects.len(), 2);
}

#[test]
fn cli_magnifier_sets_and_updates_one_fixed_position() {
    let mut state = AppState::default();
    let mut fades = Vec::new();
    let mut position = None;

    for (x, y, zoom) in [(100, 200, 2.0), (300, 400, 3.5)] {
        apply_command(
            &mut state,
            &mut fades,
            &mut position,
            &CliCommand::Magnifier { x, y, zoom },
        )
        .unwrap();
    }

    assert!(state.magnifier_active);
    assert_eq!(state.zoom_level, 3.5);
    assert_eq!(position, Some((300, 400)));
}

#[test]
fn cli_clear_matches_escape_and_clears_fades_and_fixed_magnifier() {
    let mut state = AppState {
        drawing: DrawingState::Drawing {
            start: (10, 20),
            current: (30, 40),
        },
        pinned_active: true,
        spotlight_active: true,
        magnifier_active: true,
        pinned_rects: vec![crate::state::PinnedRect {
            x0: 0,
            y0: 0,
            x1: 100,
            y1: 100,
            spotlight: false,
        }],
        ..Default::default()
    };
    let mut fades = vec![FadingRect {
        rect: (0, 0, 20, 20),
        started_at: Instant::now(),
    }];
    let mut position = Some((50, 50));

    apply_command(
        &mut state,
        &mut fades,
        &mut position,
        &CliCommand::Clear,
    )
    .unwrap();

    assert_eq!(state.drawing, DrawingState::Armed);
    assert!(state.pinned_rects.is_empty());
    assert!(!state.pinned_active);
    assert!(!state.spotlight_active);
    assert!(!state.magnifier_active);
    assert!(fades.is_empty());
    assert_eq!(position, None);
}

#[test]
fn cli_rect_partially_outside_desktop_is_clipped_by_existing_renderer() {
    let mut state = AppState::default();
    let mut fades = Vec::new();
    let mut position = None;

    apply_command(
        &mut state,
        &mut fades,
        &mut position,
        &CliCommand::Rect {
            x1: -2000,
            y1: 10,
            x2: -1800,
            y2: 100,
        },
    )
    .unwrap();

    assert_eq!(state.pinned_rects.len(), 1);
    assert_eq!(state.pinned_rects[0].x0, -2000);
    assert_eq!(state.pinned_rects[0].x1, -1800);
}

#[test]
fn cli_geometry_outside_desktop_is_rejected_without_mutation() {
    let original = AppState::default();
    let mut state = original.clone();
    let mut fades = Vec::new();
    let mut position = None;

    let rect_error = apply_command(
        &mut state,
        &mut fades,
        &mut position,
        &CliCommand::Rect {
            x1: 3000,
            y1: 100,
            x2: 3100,
            y2: 200,
        },
    )
    .unwrap_err();
    assert!(rect_error.is_code("outside_desktop"));

    let magnifier_error = apply_command(
        &mut state,
        &mut fades,
        &mut position,
        &CliCommand::Magnifier {
            x: 3000,
            y: 100,
            zoom: 2.0,
        },
    )
    .unwrap_err();
    assert!(magnifier_error.is_code("outside_desktop"));
    assert_eq!(state, original);
    assert_eq!(position, None);
}
```

- [ ] **Step 2: Run the focused tests and verify failure**

```powershell
cargo test -j 1 overlay::tests::cli_
```

Expected: compilation fails because `apply_cli_command` does not exist.

- [ ] **Step 3: Implement the pure application helper**

Add imports for `CliCommand`, `CommandEnvelope`, `CommandError`, and `PinnedRect`, then add this helper near the existing fade helpers:

```rust
fn apply_cli_command(
    state: &mut AppState,
    fades: &mut Vec<FadingRect>,
    magnifier_position: &mut Option<(i32, i32)>,
    desktop: (i32, i32, i32, i32),
    command: &CliCommand,
) -> Result<(), CommandError> {
    let (left, top, right, bottom) = desktop;
    match command {
        CliCommand::Rect { x1, y1, x2, y2 } => {
            let (x0, y0, x1, y1) = normalize_rect((*x1, *y1), (*x2, *y2));
            if x1 <= left || x0 >= right || y1 <= top || y0 >= bottom {
                return Err(CommandError::new(
                    "outside_desktop",
                    "rectangle does not intersect the virtual desktop",
                ));
            }
            state.pinned_rects.push(PinnedRect {
                x0,
                y0,
                x1,
                y1,
                spotlight: false,
            });
        }
        CliCommand::Magnifier { x, y, zoom } => {
            if *x < left || *x >= right || *y < top || *y >= bottom {
                return Err(CommandError::new(
                    "outside_desktop",
                    "magnifier center is outside the virtual desktop",
                ));
            }
            state.magnifier_active = true;
            state.zoom_level = *zoom;
            *magnifier_position = Some((*x, *y));
        }
        CliCommand::Clear => {
            *state = process_event(state, &InputEvent::EscapePressed);
            fades.clear();
            *magnifier_position = None;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run the application tests and verify green**

```powershell
cargo test -j 1 overlay::tests::cli_
```

Expected: all six CLI application tests pass.

- [ ] **Step 5: Write failing tests for constructor state and manual magnifier handoff**

Replace the existing `app_starts_without_fades` test with the first test below, then add the two helper tests around input-event preprocessing:

```rust
#[test]
fn app_starts_without_cli_visual_state() {
    let (_input_tx, input_rx) = std::sync::mpsc::channel();
    let (_config_tx, config_rx) = std::sync::mpsc::channel();
    let (_command_tx, command_rx) = std::sync::mpsc::channel();

    let app = App::new(
        input_rx,
        config_rx,
        command_rx,
        4,
        ColorMode::Rainbow,
        "Alt".into(),
    );

    assert!(app.fading_rects.is_empty());
    assert_eq!(app.magnifier_position, None);
    assert_eq!(app.virtual_desktop_rect, (0, 0, 1920, 1080));
}

#[test]
fn manual_digit_three_clears_cli_magnifier_position() {
    let mut position = Some((800, 450));
    clear_cli_magnifier_for_manual_event(&mut position, &InputEvent::DigitPressed(3));
    assert_eq!(position, None);
}

#[test]
fn unrelated_input_preserves_cli_magnifier_position() {
    let mut position = Some((800, 450));
    clear_cli_magnifier_for_manual_event(&mut position, &InputEvent::MouseMove { x: 1, y: 2 });
    assert_eq!(position, Some((800, 450)));
}
```

- [ ] **Step 6: Run these tests and verify the red state**

```powershell
cargo test -j 1 overlay::tests::app_starts_without_cli_visual_state
cargo test -j 1 overlay::tests::manual_digit_three_clears_cli_magnifier_position
```

Expected: compilation fails because the constructor signature, fields, and helper are not implemented.

- [ ] **Step 7: Wire command state into `App` and `about_to_wait`**

Make these targeted changes:

Insert these fields into the existing `App` struct without moving or renaming its other fields:

```rust
command_rx: Receiver<CommandEnvelope>,
magnifier_position: Option<(i32, i32)>,
virtual_desktop_rect: (i32, i32, i32, i32),
```

Update `App::new` to accept `command_rx` after `config_rx` and initialize:

```rust
command_rx,
fading_rects: Vec::new(),
magnifier_position: None,
virtual_desktop_rect: (0, 0, 1920, 1080),
```

After calculating `left`, `top`, `right`, and `bottom` in `resumed`, cache them before creating the window:

```rust
self.virtual_desktop_rect = (left, top, right, bottom);
```

Add and call this helper before `process_event` for each physical input:

```rust
fn clear_cli_magnifier_for_manual_event(
    magnifier_position: &mut Option<(i32, i32)>,
    event: &InputEvent,
) {
    if matches!(event, InputEvent::DigitPressed(3)) {
        *magnifier_position = None;
    }
}
```

Drain command envelopes before the existing input-event loop:

```rust
while let Ok(envelope) = self.command_rx.try_recv() {
    let result = apply_cli_command(
        &mut self.state,
        &mut self.fading_rects,
        &mut self.magnifier_position,
        self.virtual_desktop_rect,
        &envelope.command,
    );
    crate::hook::MAGNIFIER_ACTIVE.store(
        self.state.magnifier_active,
        std::sync::atomic::Ordering::Relaxed,
    );
    let _ = envelope.reply_tx.send(result);
}
```

Update magnifier rendering so the fixed CLI position wins without moving the cursor:

```rust
let cursor_pos = if let Some(position) = self.magnifier_position {
    position
} else {
    let mut point = windows::Win32::Foundation::POINT { x: 0, y: 0 };
    let _ = windows::Win32::UI::WindowsAndMessaging::GetPhysicalCursorPos(&mut point);
    (point.x, point.y)
};
mag.render(
    cursor_pos,
    self.state.zoom_level,
    &self.color_mode,
    time_offset,
);
```

Update `run_overlay` and its constructor call:

```rust
pub fn run_overlay(
    event_loop: EventLoop<()>,
    input_rx: Receiver<InputEvent>,
    config_rx: Receiver<AppConfig>,
    command_rx: Receiver<CommandEnvelope>,
    border_width: i32,
    color_mode: ColorMode,
    modifier_name: String,
) {
    let mut app = App::new(
        input_rx,
        config_rx,
        command_rx,
        border_width,
        color_mode,
        modifier_name,
    );
    event_loop.run_app(&mut app).expect("Event loop error");
}
```

In `main.rs`, create and pass a disconnected command channel so this milestone compiles before the IPC server is introduced:

```rust
let (_command_tx, command_rx) = mpsc::channel::<crate::cli::CommandEnvelope>();
```

Pass `command_rx` immediately after `config_rx` in the existing `run_overlay` call. Task 5 removes the underscore and gives `command_tx` to the pipe server.

- [ ] **Step 8: Run focused overlay and state suites**

```powershell
cargo test -j 1 overlay::tests::cli_
cargo test -j 1 overlay::tests::app_starts_without_cli_visual_state
cargo test -j 1 overlay::tests::manual_digit_three_clears_cli_magnifier_position
cargo test -j 1 state::tests
```

Expected: all pass. Existing manual state transitions remain unchanged.

- [ ] **Step 9: Commit resident command application**

```powershell
git add src/overlay.rs src/main.rs
git commit -m "feat(cli): apply commands in overlay"
```

---

## Milestone 3: Windows Named-Pipe Transport

### Task 4: Add acknowledged named-pipe client/server

**Files:**
- Create: `src/ipc.rs`
- Modify: `src/main.rs:3-14`
- Modify: `Cargo.toml:12-31`
- Test: `src/ipc.rs` inline Windows-only tests

**Interfaces:**
- Consumes: CLI codec and `CommandEnvelope` from Tasks 1-2, plus `EventLoopProxy<()>`.
- Produces: `PIPE_NAME`, `send_command(pipe_name: &str, command: &CliCommand, deadline: Instant) -> Result<(), CommandError>`, and `start_server(pipe_name: String, command_tx: Sender<CommandEnvelope>, proxy: EventLoopProxy<()>) -> JoinHandle<()>`.
- Task 5 calls only these public interfaces; raw HANDLE helpers remain private.

- [ ] **Step 1: Enable the required windows-rs feature and add the module**

Add `"Win32_System_Pipes"` to the existing `windows` feature list in `Cargo.toml`. Add this declaration in `src/main.rs`:

```rust
#[cfg(windows)]
mod ipc;
```

Do not alter the windows crate version or add a dependency.

- [ ] **Step 2: Write failing named-pipe round-trip tests**

Create `src/ipc.rs` with a Windows-only test module. The production names are intentionally unresolved for the red run:

```rust
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
}
```

- [ ] **Step 3: Run the IPC test and verify it is red**

```powershell
cargo test -j 1 ipc::tests::named_pipe_round_trip_waits_for_application_reply
```

Expected: compilation fails because the named-pipe helpers and public transport functions do not exist.

- [ ] **Step 4: Implement handle ownership and bounded line I/O**

Start `src/ipc.rs` with these imports and helpers. They match windows crate 0.58 signatures: `CreateNamedPipeW` returns `HANDLE`, `ConnectNamedPipe` returns `Result<()>`, and `WaitNamedPipeW` returns `BOOL`.

```rust
use crate::cli::{
    decode_request, decode_response, encode_request, encode_response, CliCommand, CommandEnvelope,
    CommandError, MAX_WIRE_BYTES,
};
use std::sync::mpsc::Sender;
use std::time::Instant;
use windows::core::{Error as WindowsError, HRESULT, PCWSTR};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
    GENERIC_READ, GENERIC_WRITE,
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
            ReadFile(
                handle,
                Some(&mut chunk[..capacity]),
                Some(&mut read),
                None,
            )
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
```

- [ ] **Step 5: Implement one server connection, the serialized server loop, and client send**

Add these functions. Keep one server thread; do not create a worker pool.

```rust
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
        Err(io_error("pipe_create", WindowsError::from_win32().to_string()))
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
        Err(error) if same_win32_error(&error, ERROR_PIPE_CONNECTED) => {}
        Err(error) => return Err(io_error("pipe_connect", error.to_string())),
    }

    let request = read_line(pipe.0);
    let result = match request.and_then(|frame| decode_request(&frame)) {
        Ok(command) => {
            let (reply_tx, reply_rx) = std::sync::mpsc::channel();
            command_tx
                .send(CommandEnvelope { command, reply_tx })
                .map_err(|_| io_error("resident_stopped", "resident event loop stopped"))?;
            wake();
            reply_rx
                .recv()
                .map_err(|_| io_error("resident_stopped", "resident reply channel closed"))?
        }
        Err(error) => Err(error),
    };

    write_all(pipe.0, encode_response(&result).as_bytes())?;
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
                return Err(io_error("pipe_not_found", "HoldRect command pipe is not ready"));
            }
            Err(error) if same_win32_error(&error, ERROR_PIPE_BUSY) => {
                let timeout = remaining_millis(deadline)?;
                let available = unsafe { WaitNamedPipeW(PCWSTR(name.as_ptr()), timeout) };
                if !available.as_bool() {
                    return Err(io_error("pipe_timeout", "timed out waiting for busy pipe"));
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
```

- [ ] **Step 6: Run the first IPC tests and fix only empirical windows-rs signature mismatches**

```powershell
cargo test -j 1 ipc::tests::named_pipe_round_trip_waits_for_application_reply
cargo test -j 1 ipc::tests::named_pipe_propagates_application_error
cargo test -j 1 ipc::tests::absent_pipe_returns_distinct_error
```

Expected: all pass. If windows crate 0.58 reports a type mismatch, use the compiler and the locally installed `windows-0.58.0` generated signatures; do not change transport behavior or add a wrapper dependency.

- [ ] **Step 7: Add failing resilience tests**

Add tests that create the server handle synchronously before starting clients, so no arbitrary sleeps are used:

```rust
#[test]
fn request_split_across_writes_is_reassembled() {
    let pipe_name = unique_pipe();
    let server_pipe = create_server_pipe(&pipe_name).unwrap();
    let server = std::thread::spawn(move || {
        match unsafe { ConnectNamedPipe(server_pipe.0, None) } {
            Ok(()) => {}
            Err(error) if same_win32_error(&error, ERROR_PIPE_CONNECTED) => {}
            Err(error) => panic!("{error}"),
        }
        let frame = read_line(server_pipe.0).unwrap();
        assert_eq!(frame, b"clear\n");
    });

    let client = open_client(
        &pipe_name,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
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
    let first_client = open_client(
        &pipe_name,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
    write_all(first_client.0, &vec![b'x'; MAX_WIRE_BYTES]).unwrap();
    write_all(first_client.0, b"\n").unwrap();
    let response = read_line(first_client.0).unwrap();
    assert!(decode_response(&response)
        .unwrap_err()
        .is_code("request_too_large"));
    first_server.join().unwrap().unwrap();

    let second_pipe = create_server_pipe(&pipe_name).unwrap();
    let second_server = std::thread::spawn(move || {
        serve_connected_pipe(second_pipe, command_tx, || {})
    });
    let app = std::thread::spawn(move || {
        command_rx
            .recv()
            .unwrap()
            .reply_tx
            .send(Ok(()))
            .unwrap();
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
            command_rx
                .recv()
                .unwrap()
                .reply_tx
                .send(Ok(()))
                .unwrap();
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

    let client = open_client(
        &pipe_name,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
    write_all(client.0, b"cl").unwrap();
    drop(client);

    let error = server.join().unwrap().unwrap_err();
    assert!(error.is_code("pipe_read"));
}

#[test]
fn client_disconnect_before_response_does_not_panic_server() {
    let pipe_name = unique_pipe();
    let pipe = create_server_pipe(&pipe_name).unwrap();
    let (command_tx, command_rx) = mpsc::channel();
    let server = std::thread::spawn(move || serve_connected_pipe(pipe, command_tx, || {}));
    let app = std::thread::spawn(move || {
        command_rx
            .recv()
            .unwrap()
            .reply_tx
            .send(Ok(()))
            .unwrap();
    });

    let client = open_client(
        &pipe_name,
        Instant::now() + Duration::from_secs(2),
    )
    .unwrap();
    write_all(client.0, b"clear\n").unwrap();
    drop(client);

    app.join().unwrap();
    let _connection_result = server.join().unwrap();
}
```

- [ ] **Step 8: Run all IPC tests and the full focused CLI suite**

```powershell
cargo test -j 1 ipc::tests
cargo test -j 1 cli::tests
```

Expected: all pass, including split reads, oversize recovery, structured errors, and sequential clients.

- [ ] **Step 9: Commit the transport**

```powershell
git add Cargo.toml src/main.rs src/ipc.rs
git commit -m "feat(cli): add named pipe transport"
```

---

## Milestone 4: Single-Executable Startup and Delivery

### Task 5: Route process modes, auto-start the daemon, and wire IPC

**Files:**
- Modify: `src/main.rs:1-94`
- Modify: `src/overlay.rs:962-971`
- Test: `src/main.rs` inline test module

**Interfaces:**
- Consumes: `parse_startup_args`, `StartupMode`, `CliCommand`, `CommandError`, `ipc::send_command`, `ipc::start_server`, and the updated `run_overlay`.
- Produces: `deliver_with_auto_start<Send, Spawn>(command: &CliCommand, deadline: Instant, retry_delay: Duration, send: Send, spawn: Spawn) -> Result<(), CommandError>`, `run_client(command: &CliCommand) -> Result<(), CommandError>`, and `run_resident(first_launch: bool)`.
- No code outside `main.rs` calls these process-orchestration helpers.

- [ ] **Step 1: Write failing auto-start decision tests**

Add this test module to `src/main.rs`. It checks the nontrivial retry branch without spawning a real daemon or sleeping:

```rust
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
```

- [ ] **Step 2: Run the auto-start test and verify failure**

```powershell
cargo test -j 1 tests::missing_daemon_spawns_once_then_retries
```

Expected: compilation fails because `deliver_with_auto_start` is undefined.

- [ ] **Step 3: Implement the tested retry helper and real client path**

Add these helpers above `main`:

```rust
use crate::cli::{CliCommand, CommandError, StartupMode};
use std::time::{Duration, Instant};

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
```

- [ ] **Step 4: Run the retry tests and verify green**

```powershell
cargo test -j 1 tests::ready_daemon_does_not_spawn
cargo test -j 1 tests::missing_daemon_spawns_once_then_retries
cargo test -j 1 tests::non_missing_error_does_not_spawn
cargo test -j 1 tests::missing_daemon_respects_deadline
```

Expected: all four pass without spawning a process.

- [ ] **Step 5: Refactor `main` into explicit client and resident routes**

Replace the ad-hoc `--mem-report` scan with exact mode parsing before `try_acquire()`:

```rust
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
```

Move the current resident startup body into `run_resident(first_launch: bool)`. Preserve its order, with these exact behavioral changes:

```rust
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
```

The CLI route returns before `run_resident`, so it never calls `try_acquire()`.

- [ ] **Step 6: Run routing, IPC, overlay, and single-instance tests**

```powershell
cargo test -j 1 cli::tests
cargo test -j 1 tests::missing_daemon_spawns_once_then_retries
cargo test -j 1 ipc::tests
cargo test -j 1 overlay::tests::cli_
cargo test -j 1 single_instance::tests
```

Expected: all pass. The existing single-instance tests still own only resident mutex behavior.

- [ ] **Step 7: Build the first end-to-end CLI executable**

```powershell
cargo build -j 1
```

Expected: exit code `0` and `target/debug/holdrect.exe` exists.

Perform this bounded smoke test from PowerShell:

```powershell
$exe = Resolve-Path .\target\debug\holdrect.exe
& $exe clear
if ($LASTEXITCODE -ne 0) { throw "clear failed: $LASTEXITCODE" }
& $exe rect 100 100 500 400
if ($LASTEXITCODE -ne 0) { throw "rect failed: $LASTEXITCODE" }
& $exe magnifier 800 450 3
if ($LASTEXITCODE -ne 0) { throw "magnifier failed: $LASTEXITCODE" }
& $exe clear
if ($LASTEXITCODE -ne 0) { throw "final clear failed: $LASTEXITCODE" }
```

Expected visible evidence: one resident tray process auto-starts; the rectangle remains after the client exits; the magnifier is centered at `(800, 450)` without moving the cursor; final clear removes both. Each successful command returns `0` only after application.

- [ ] **Step 8: Commit process integration**

```powershell
git add src/main.rs src/overlay.rs
git commit -m "feat(cli): auto-start resident commands"
```

---

## Milestone 5: User Documentation and Final Validation

### Task 6: Document and verify the complete CLI

**Files:**
- Modify: `README.md` after the Quick Start/shortcut content
- Verify: all changed Rust modules and the built executable

**Interfaces:**
- Consumes: the final public command syntax and behavior.
- Produces: user-facing CLI documentation and final verification evidence; no new runtime interface.

- [ ] **Step 1: Add the exact CLI documentation**

Add this section to `README.md`:

````markdown
## CLI / AI Control

The same `holdrect.exe` can control the resident overlay from scripts and AI tools. A command automatically starts HoldRect when it is not already running.

```powershell
holdrect rect 100 200 500 400
holdrect magnifier 800 450
holdrect magnifier 800 450 3
holdrect clear
```

- `rect x1 y1 x2 y2` adds a rectangle that remains until `clear`.
- `magnifier x y [zoom]` shows one fixed magnifier; zoom defaults to `2.0` and accepts `1.5` through `8.0`.
- `clear` removes all pinned/fading rectangles, cancels the active drawing, and hides the magnifier.

Coordinates are signed physical pixels in the Windows virtual-desktop coordinate space, so monitors left of the primary display can use negative `x` values.
````

- [ ] **Step 2: Format and run the complete test suite with one job**

```powershell
cargo fmt --all
cargo fmt --all -- --check
cargo test -j 1
```

Expected: both commands exit `0`; no ignored failure or partial suite is accepted.

- [ ] **Step 3: Run release-shaped build and regression checks**

```powershell
cargo build --release -j 1
.\target\release\holdrect.exe --mem-report
```

Expected: release build exits `0`, memory report still prints, and no GUI/tray process is created by `--mem-report`.

Then verify these existing flows manually against the release executable:

1. No arguments: one tray icon appears and the normal `FirstLaunch` popup appears.
2. Starting a second no-argument instance: it exits and the existing process shows “Already running”.
3. `Alt` + drag: transient rectangle still draws and fades.
4. `Alt+1` + drag: pinned rectangle remains.
5. `Alt+2` + drag: Spotlight remains correct.
6. `Alt+3`: magnifier follows the cursor and the wheel changes zoom.
7. After a CLI fixed magnifier, pressing `Alt+3` returns it to manual cursor-following semantics.
8. Editing `~/.holdrect/config.toml`: hot reload still applies.
9. Tray Exit: the resident process terminates and releases the named pipe and mutex.

- [ ] **Step 4: Run the final CLI acceptance sequence**

```powershell
$exe = Resolve-Path .\target\release\holdrect.exe
& $exe clear
& $exe rect 100 100 500 400
& $exe rect -800 120 -300 420
& $exe magnifier 800 450
& $exe magnifier 900 500 3
& $exe clear
if ($LASTEXITCODE -ne 0) { throw "HoldRect CLI acceptance failed" }
```

Expected: one daemon auto-starts, both valid rectangles coexist when their coordinates intersect the actual virtual desktop, repeated magnifier commands move one lens, every command prints `OK` when console attachment is available, and final clear removes all affected visuals. On a machine without a monitor at negative coordinates, the negative rectangle must return `outside_desktop` nonzero instead of mutating state; rerun that case with coordinates from an attached secondary monitor for the positive path.

- [ ] **Step 5: Check the final diff and commit documentation**

```powershell
git diff --check
git status --short
git add README.md
git commit -m "docs: document HoldRect CLI"
```

Expected: only intended task files are present before the commit, `git diff --check` is clean, and the documentation commit succeeds.

- [ ] **Step 6: Review gate before branch completion**

Stop and return control to the parent orchestrator. The parent launches a fresh `ultra-reviewer` for correctness/regressions, TDD coverage, Win32 handle/error paths, process lifecycle, simplicity, and the actual final diff. If fixes are accepted, the parent launches exactly one fix worker, reruns affected focused tests plus `cargo test -j 1`, and repeats focused review only if the fixes materially change behavior. Ordinary task workers must not launch or coordinate subagents.
