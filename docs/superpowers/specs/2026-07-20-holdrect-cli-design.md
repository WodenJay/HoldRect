# HoldRect CLI Design

## Goal

Add a Windows-first command-line control surface so AI agents and scripts can draw persistent highlight rectangles, show a fixed magnifier at physical screen coordinates, and clear the screen. The CLI must reuse the resident HoldRect renderer, preserve the existing manual interaction model, keep the single-file distribution, and add no third-party dependency.

## Confirmed Behavior

- `holdrect.exe` remains the only distributed executable.
- Running `holdrect` without arguments starts the resident tray application exactly as it does today.
- Running a CLI subcommand acts as a client. If no resident instance is ready, the client starts `holdrect.exe --daemon`, waits for it, sends the command, and exits while the resident process remains running.
- The first iteration exposes exactly three visual commands: rectangle, magnifier, and clear.
- CLI rectangles persist until `clear`; repeated rectangle commands may create multiple simultaneous rectangles.
- The magnifier is fixed at the supplied point. Its source point and lens center are the same, matching the current cursor-following magnifier geometry without moving the real cursor.
- Repeating the magnifier command moves the existing magnifier and may update its zoom. It does not create another magnifier window.
- `clear` follows the existing Escape clearing semantics: cancel the active drawing, remove pinned and fading rectangles, reset per-rectangle flags, and hide the magnifier. It does not exit HoldRect or change configuration.
- A CLI command reports success only after the resident event loop has accepted and applied its state change. It does not wait for a subsequent frame to be submitted to Windows.
- Existing modifier, mouse, Pin, Spotlight, popup, tray, configuration, and memory-report behavior remains unchanged.

## Public Command Interface

```text
holdrect rect <x1> <y1> <x2> <y2>
holdrect magnifier <x> <y> [zoom]
holdrect clear
```

Two rectangle corner points are passed as four scalar arguments: `(x1, y1)` and `(x2, y2)`. For example:

```powershell
holdrect rect 100 200 500 400
```

All coordinates are signed `i32` physical pixels in the Windows virtual-desktop coordinate space. Negative coordinates are valid when a monitor is positioned left of or above the virtual origin. The rectangle corners may be supplied in either direction and are normalized before storage.

The optional magnifier zoom defaults to the current HoldRect default of `2.0`. Accepted zoom values are inclusive from `1.5` through `8.0`.

Successful commands print `OK` when a parent console is available and return exit code `0`. Invalid arguments, startup failures, IPC failures, daemon rejections, and timeouts print a concise diagnostic when possible and return a nonzero exit code. Exit status remains authoritative when no console can be attached.

The existing `--mem-report` path remains an immediate, non-daemon command. `--daemon` is an internal startup mode used by the CLI client rather than a documented visual command.

## Process Modes and Startup

Argument routing happens before the existing normal GUI initialization:

1. `--mem-report` preserves its current behavior and exits.
2. A visual subcommand enters CLI client mode.
3. `--daemon` enters resident mode without showing the `FirstLaunch` status popup.
4. No arguments enter normal resident mode and preserve the `FirstLaunch` popup.
5. Unknown or malformed arguments return a usage error without starting the resident process.

CLI client mode returns before the existing single-instance check. Only resident mode calls `try_acquire()` and holds the mutex as it does today.

Client mode first attempts to connect to the resident command pipe. If the pipe does not exist, it spawns `current_exe()` with `--daemon` and retries until either the pipe becomes ready or five seconds elapse. The existing named mutex remains the authority for single-instance enforcement. Concurrent clients may race to spawn a daemon, but only one resident process survives; every client waits for the same pipe and sends its own command.

The daemon spawned by a command remains a normal tray application after that command completes. It starts the input hook, configuration watcher, tray, overlay, and command server exactly once. If the spawning client later times out, it exits nonzero without terminating a daemon that may have started successfully.

## Command Model

A new pure command type represents validated intent:

```text
CliCommand::Rect { x1, y1, x2, y2 }
CliCommand::Magnifier { x, y, zoom }
CliCommand::Clear
```

`src/cli.rs` owns this type, raw standard-library argument parsing, validation shared by client and server, and the small text wire format. No `clap`, serialization crate, or speculative command framework is introduced.

A resident-only `CommandEnvelope` contains one `CliCommand` and a single-use `std::sync::mpsc::Sender<Result<(), CommandError>>` created for that request. It is not part of the cross-process protocol. The envelope lets the pipe listener wait without blocking the UI thread and lets the event loop acknowledge the exact command after applying it.

## IPC Transport

`src/ipc.rs` owns a Windows named-pipe client and server using the existing `windows` crate. `Cargo.toml` enables the crate's required `Win32_System_Pipes` feature but adds no dependency:

```text
\\.\pipe\HoldRect
```

The server runs on one background thread. It blocks while idle, accepts one request per connection, sends one response, disconnects, and accepts the next client. Commands are intentionally serialized; expected traffic is tiny, and a worker pool or asynchronous runtime would add memory and complexity without benefit.

The pipe uses byte mode with a newline-terminated UTF-8 request no larger than 512 bytes:

```text
rect 100 200 500 400\n
magnifier 800 450 3\n
clear\n
```

Responses are one newline-terminated UTF-8 line:

```text
OK\n
ERR <code> <message>\n
```

The server rejects remote clients, relies on the creator process's Windows security descriptor so unprivileged cross-user clients cannot issue write commands, and validates every request again rather than trusting the client executable. The fixed request limit bounds allocation and prevents an unterminated or malicious request from growing indefinitely.

After decoding a request, the pipe thread sends a `CommandEnvelope` through a dedicated command channel and wakes the existing winit event loop with its proxy. It waits for the envelope reply, writes `OK` or `ERR`, then closes that client connection. It never reads or mutates overlay state directly.

## Resident Application Integration

`main.rs` creates the command channel beside the existing input and configuration channels, starts the named-pipe listener after resident ownership is established, and passes the command receiver into `overlay::App`.

The command channel remains separate from `InputEvent`. This avoids putting reply senders into the existing cloneable, comparable input enum and preserves the pure manual-input state machine and its large test suite.

`App::about_to_wait` drains pending command envelopes on the event-loop thread. For each envelope it applies one command atomically, updates `hook::MAGNIFIER_ACTIVE` when necessary, and only then sends the reply. Commands preserve their own FIFO order. Exact ordering against physical input arriving at the same instant is unspecified; both are serialized by the event-loop thread without shared mutable state or locking.

`App` also stores the current virtual-desktop bounds calculated in `resumed()`. Command application uses these cached physical-pixel bounds for resident-side geometry validation, so the CLI process does not need monitor APIs or duplicate DPI logic.

### Rectangle

`rect` normalizes the two corners and appends one existing `PinnedRect` with `spotlight: false`. It does not toggle or consume the user's current `pinned_active` or `spotlight_active` flags and does not interrupt a manual drawing. Existing overlay rendering, rainbow animation, clipping, and multi-monitor offsets draw it without a new rendering path.

### Magnifier

`App` gains one `magnifier_position: Option<(i32, i32)>` field:

- `Some(position)` means a CLI command owns a fixed magnifier center.
- `None` means the existing magnifier follows the physical cursor.

The magnifier command sets the fixed position, activates the existing `AppState.magnifier_active`, and sets `zoom_level`. Rendering chooses the fixed position when present and otherwise calls `GetPhysicalCursorPos` as it does today.

When `about_to_wait` processes a pending manual `DigitPressed(3)`, it clears `magnifier_position` before calling `process_event`. Turning the magnifier on manually therefore restores cursor-following behavior without moving this CLI-only field into the pure input state machine. Scroll-wheel zoom continues to update the shared zoom level while the magnifier is active.

### Clear

`clear` reuses the existing Escape state transition, clears `fading_rects`, and clears `magnifier_position`. This guarantees the CLI clear behavior stays aligned with the keyboard Escape behavior without duplicating all of its state rules.

## Validation and Error Handling

Client-side parsing rejects:

- missing or extra arguments;
- unknown commands;
- non-integer coordinates;
- coordinates outside the `i32` range;
- a rectangle with zero width or zero height after normalization;
- a non-finite zoom or zoom outside `1.5..=8.0`.

The server repeats protocol and value validation. Before applying a command, the resident process checks current virtual-desktop bounds:

- a rectangle must intersect the virtual desktop; partial off-screen geometry is accepted and naturally clipped by the renderer;
- a magnifier center must lie inside the virtual desktop.

Validation failure leaves all application state unchanged. A malformed UTF-8 request, missing newline, oversized request, closed client, invalid command, or unavailable reply channel produces an error response when the connection still permits one and never terminates the resident application.

Startup distinguishes an absent pipe from a busy pipe. A busy pipe is waited on within the same five-second deadline rather than starting another daemon. Spawn failure, timeout, or connection loss produces a nonzero client exit.

## TDD and Verification Strategy

Implementation follows red-green-refactor. Production behavior is not added before the corresponding failing tests demonstrate the missing behavior.

### CLI parser and validation tests

- Parse each valid command.
- Parse negative virtual-desktop coordinates.
- Normalize either corner order.
- Apply the default magnifier zoom of `2.0`.
- Accept zoom boundaries `1.5` and `8.0`.
- Reject missing and extra arguments.
- Reject unknown commands.
- Reject non-integer and overflowing coordinates.
- Reject zero-width and zero-height rectangles.
- Reject NaN, infinity, and out-of-range zoom.
- Preserve the existing `--mem-report`, normal-resident, and internal-daemon routing decisions.

### Wire protocol tests

- Encode and decode every command.
- Encode `OK` and structured `ERR` responses.
- Handle requests split across several reads.
- Reject invalid UTF-8.
- Reject missing newline at the request limit.
- Reject requests over 512 bytes.
- Reject trailing tokens rather than ignoring them.

### Command application tests

- Append a normalized non-Spotlight pinned rectangle.
- Preserve an active manual drawing and its per-rectangle flags when appending a CLI rectangle.
- Retain multiple CLI and manual pinned rectangles together.
- Activate a fixed-position magnifier with default or explicit zoom.
- Move and reconfigure the existing magnifier on repeated commands.
- Clear pinned rectangles, fades, active drawing state, per-rectangle flags, magnifier activation, and fixed magnifier position.
- Restore cursor-following mode after manual `DigitPressed(3)`.
- Leave state unchanged when resident-side desktop validation fails.
- Send success only after state mutation and an error when mutation is rejected.

### Windows named-pipe integration tests

Tests use unique pipe names so they cannot contact a real HoldRect instance:

- Complete one client/server `OK` round trip.
- Propagate a structured daemon error to the client.
- Serve sequential clients without restarting the listener.
- Preserve each response under concurrent client attempts.
- Handle a client disconnect during request and response.
- Time out cleanly when no server appears.
- Reject an oversized request and remain available for the next client.

### Focused and final verification

Cargo commands always use one job. Run the smallest relevant module tests during each TDD step, then run:

```powershell
cargo test -j 1
```

Final user-path validation uses the built executable, not only unit tests:

```powershell
holdrect clear
holdrect rect 100 100 500 400
holdrect rect -800 120 -300 420
holdrect magnifier 800 450
holdrect magnifier 900 500 3
holdrect clear
```

Observe that the first command can auto-start one resident process, multiple rectangles coexist, the fixed magnifier moves without moving the cursor, every successful invocation returns only after application, and clear removes all affected visuals. Also verify normal no-argument startup, tray exit, manual Alt-drag, Pin, Spotlight, cursor-following magnifier, configuration hot reload, and `--mem-report` for regressions.

## Resource Model

The resident process adds one blocking pipe thread, one command receiver, and at most one in-flight request buffer per serialized connection. There is no polling loop, network listener, async runtime, command history, object registry, or pixel copy. CLI rectangles reuse the existing compact `PinnedRect` vector, and the fixed magnifier adds only one optional coordinate pair.

## Non-Goals

- No rectangle IDs, listing, querying, updating, or single-object deletion.
- No timed rectangle expiry; CLI rectangles remain until `clear`.
- No CLI Spotlight, color, border-width, animation, popup, tray, config, or exit commands.
- No multiple magnifier windows.
- No normalized, percentage, logical-DPI, monitor-relative, or screenshot-relative coordinates.
- No cursor movement or synthetic keyboard/mouse input.
- No guarantee of ordering between a CLI command and a simultaneous physical input event.
- No rendered-frame acknowledgment; success means state applied by the resident event loop.
- No TCP server, HTTP API, JSON-RPC, plugin system, or new serialization/argument-parsing dependency.
- No macOS/Linux IPC implementation in this iteration. Their future transport may use Unix domain sockets while preserving the command model.
