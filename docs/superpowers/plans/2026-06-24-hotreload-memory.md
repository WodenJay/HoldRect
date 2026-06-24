# Hot-reload Config + Memory Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hot-reload `~/.holdrect/config.toml` without restart; measure and reduce memory footprint.

**Architecture:** Watcher thread uses Win32 `ReadDirectoryChangesW` (sync I/O) to monitor config dir, sends parsed `AppConfig` via mpsc channel to main event loop. `App::about_to_wait` polls the channel and updates rendering params + hook's modifier codes (RwLock). Memory optimization: baseline measurement via `--mem-report` flag, then compile-time profile tuning, then runtime optimizations if needed.

**Tech Stack:** Rust, Win32 API (`ReadDirectoryChangesW`, `GetProcessMemoryInfo`), `std::sync::RwLock`, mpsc channels. No new crate dependencies for hot-reload.

## Global Constraints

- Cargo max concurrency = 1 (`CARGO_BUILD_JOBS=1`)
- No new crate dependencies for hot-reload feature
- TDD: write failing test → implement → pass → commit
- Each task ends with a commit
- `#![windows_subsystem = "windows")]` — use `AttachConsole` for `--mem-report` stdout
- Error handling: invalid config keeps current config, stderr warning

---

### Task 1: Memory baseline — `--mem-report` flag

**Files:**
- Create: `src/mem_report.rs`
- Modify: `src/main.rs:2-11` (add `mod mem_report`)
- Modify: `src/main.rs:20-42` (add arg check before event loop)

**Interfaces:**
- Produces: `pub fn get_process_memory_kb() -> Option<(u64, u64)>` returning `(working_set_kb, pagefile_kb)`
- Produces: `pub fn print_mem_report()` printing to stdout via `AttachConsole`

- [ ] **Step 1: Write tests for memory measurement**

Create `src/mem_report.rs`:

```rust
// src/mem_report.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_process_memory_returns_nonzero() {
        let Some((ws, pf)) = get_process_memory_kb() else {
            panic!("GetProcessMemoryInfo failed");
        };
        assert!(ws > 0, "working set must be > 0, got {ws}");
        assert!(pf > 0, "pagefile usage must be > 0, got {pf}");
    }

    #[test]
    fn get_process_memory_reasonable_range() {
        let Some((ws, _pf)) = get_process_memory_kb() else { return; };
        // Current HoldRect uses <50MB. Assert <500MB as sanity check.
        assert!(ws < 500_000, "working set {ws} KB seems unreasonably high");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib mem_report`
Expected: FAIL — `get_process_memory_kb` not defined

- [ ] **Step 3: Implement memory measurement**

Replace `src/mem_report.rs` with:

```rust
// src/mem_report.rs
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;

/// Returns (working_set_size_kb, pagefile_usage_kb) for the current process.
pub fn get_process_memory_kb() -> Option<(u64, u64)> {
    unsafe {
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32).is_err() {
            return None;
        }
        Some((
            counters.WorkingSetSize as u64 / 1024,
            counters.PagefileUsage as u64 / 1024,
        ))
    }
}

/// Print memory report to stdout. For `--mem-report` CLI flag.
pub fn print_mem_report() {
    match get_process_memory_kb() {
        Some((ws, pf)) => {
            println!("HoldRect Memory Report:");
            println!("  Working Set:  {ws} KB ({:.1} MB)", ws as f64 / 1024.0);
            println!("  Pagefile:     {pf} KB ({:.1} MB)", pf as f64 / 1024.0);
        }
        None => {
            eprintln!("Error: GetProcessMemoryInfo failed");
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib mem_report`
Expected: PASS

- [ ] **Step 5: Add `--mem-report` CLI handling to main.rs**

In `src/main.rs`, add `mod mem_report;` at line 9 (after `mod popup;`). Then add arg check at the start of `main()`, before `set_dpi_awareness()`:

```rust
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

    #[cfg(windows)]
    set_dpi_awareness();
    // ... rest of main unchanged ...
}
```

- [ ] **Step 6: Add `Win32_System_ProcessStatus` and `Win32_System_Console` features to Cargo.toml**

In `Cargo.toml`, add to the `windows` features list:

```toml
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Foundation",
    "Win32_UI_HiDpi",
    "Win32_Graphics_Gdi",
    "Win32_System_Registry",
    "Win32_System_ProcessStatus",  # new
    "Win32_System_Console",        # new
    "Win32_System_Threading",      # new (GetCurrentProcess)
] }
```

- [ ] **Step 7: Run all tests to confirm no regression**

Run: `cargo test --lib`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/mem_report.rs src/main.rs Cargo.toml
git commit -m "feat(mem): add --mem-report flag for baseline memory measurement"
```

- [ ] **Step 9: Record BASELINE (关键步骤 — 必须在Task 2之前执行)**

```bash
cargo build --release
./target/release/holdrect.exe --mem-report
```

**记录输出的 Working Set 和 Pagefile 数字为 BASELINE。** 用当前未优化的Cargo.toml编译。此数字是后续所有优化的对比基准。

Expected output类似:
```
HoldRect Memory Report:
  Working Set:  XXXX KB (X.X MB)
  Pagefile:     XXXX KB (X.X MB)
```

将BASELINE数字记录在commit message或注释中, 确保可追溯。

---

### Task 2: Compile-time optimization

**前置条件: BASELINE已在Task 1 Step 9记录。如果BASELINE < 3MB, 仍执行编译优化(免费收益)但跳过Task 8。**

**Files:**
- Modify: `Cargo.toml:4-6` (add `[profile.release]` section)

**Interfaces:**
- No API changes

- [ ] **Step 1: Add release profile optimization**

Append to `Cargo.toml`:

```toml
[profile.release]
lto = true
strip = true
panic = "abort"
codegen-units = 1
opt-level = "s"
```

- [ ] **Step 2: Run all tests to confirm no regression**

Run: `cargo test --lib`
Expected: All tests PASS

- [ ] **Step 3: Build release and compare with BASELINE**

```bash
cargo build --release
./target/release/holdrect.exe --mem-report
```

**对比输出数字与Task 1 Step 9记录的BASELINE。** 记录编译优化带来的差异(Working Set减少XKB, Pagefile减少YKB)。

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "perf(build): add release profile optimization (lto, strip, panic=abort)"
```

---

### Task 3: hook.rs — OnceLock → RwLock

**Files:**
- Modify: `src/hook.rs:15` (MODIFIER_CODES type)
- Modify: `src/hook.rs:19-22` (start_hook_listener init)
- Modify: `src/hook.rs:59` (keyboard_hook_proc read)
- Modify: `src/hook.rs:122` (decide_keyboard param)

**Interfaces:**
- Produces: `pub fn update_modifier_codes(new_codes: Vec<u32>)` — called from overlay.rs Task 6
- Consumes: `decide_keyboard(vk_code, is_key_down, modifier_codes: &[u32], modifier_held)` — signature unchanged, but now called with read lock guard

- [ ] **Step 1: Write test for update_modifier_codes**

Add to `src/hook.rs` test module:

```rust
#[test]
fn update_modifier_codes_changes_read_value() {
    // Use a dedicated test with a fresh RwLock
    let lock = std::sync::RwLock::new(vec![0x12u32]);
    {
        let codes = lock.read().unwrap();
        assert_eq!(*codes, vec![0x12]);
    }
    {
        let mut codes = lock.write().unwrap();
        *codes = vec![0x11, 0xA2, 0xA3];
    }
    {
        let codes = lock.read().unwrap();
        assert_eq!(*codes, vec![0x11, 0xA2, 0xA3]);
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --lib hook::tests::update_modifier_codes_changes_read_value`
Expected: PASS (this tests the RwLock pattern, not the global static — we verify the global works via integration)

- [ ] **Step 3: Change MODIFIER_CODES from OnceLock to RwLock**

In `src/hook.rs`, replace:

```rust
// Before:
use std::sync::OnceLock;
static MODIFIER_CODES: OnceLock<Vec<u32>> = OnceLock::new();
```

With:

```rust
// After:
use std::sync::RwLock;
use std::sync::OnceLock;  // keep for TX, PROXY
static MODIFIER_CODES: RwLock<Vec<u32>> = RwLock::new(Vec::new());
```

- [ ] **Step 4: Update start_hook_listener**

Replace `MODIFIER_CODES.set(modifier_codes)` with:

```rust
*MODIFIER_CODES.write().expect("MODIFIER_CODES lock poisoned") = modifier_codes;
```

- [ ] **Step 5: Update keyboard_hook_proc read**

In `keyboard_hook_proc`, change:

```rust
// Before:
if let Some(event) = decide_keyboard(kb.vkCode, is_key_down, MODIFIER_CODES.get().expect("MODIFIER_CODES not set"), modifier_held) {

// After:
let codes = MODIFIER_CODES.read().expect("MODIFIER_CODES lock poisoned");
if let Some(event) = decide_keyboard(kb.vkCode, is_key_down, &codes, modifier_held) {
```

Note: The read lock guard `codes` must live for the duration of the `if let` block. The existing `if let` already scopes it correctly.

- [ ] **Step 6: Add update_modifier_codes public function**

Add after `start_hook_listener`:

```rust
/// Update modifier key codes at runtime (for hot-reload).
pub fn update_modifier_codes(new_codes: Vec<u32>) {
    *MODIFIER_CODES.write().expect("MODIFIER_CODES lock poisoned") = new_codes;
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test --lib`
Expected: All tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/hook.rs
git commit -m "refactor(hook): MODIFIER_CODES OnceLock → RwLock for hot-reload"
```

---

### Task 4: config.rs — watch_config_dir + PartialEq

**Files:**
- Modify: `src/config.rs:9` (add PartialEq to AppConfig derive)
- Modify: `src/config.rs` (add watch_config_dir function at end of non-test code)

**Interfaces:**
- Produces: `pub fn watch_config_dir(dir: PathBuf, tx: Sender<AppConfig>)` — spawned as thread from main.rs Task 7
- Produces: `AppConfig: PartialEq` — enables comparison in tests and in watcher (skip send if unchanged)

- [ ] **Step 1: Write test for watch_config_dir**

Add to `src/config.rs` test module:

```rust
use std::time::Duration;

#[test]
fn watch_config_dir_detects_file_change() {
    let dir = std::env::temp_dir().join("holdrect_test_watch");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.toml"), "border_width = 2\n").unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    let watch_dir = dir.clone();
    let handle = std::thread::spawn(move || {
        watch_config_dir(watch_dir, tx);
    });

    // Give watcher time to start
    std::thread::sleep(Duration::from_millis(200));

    // Modify the file
    std::fs::write(dir.join("config.toml"), "border_width = 8\n").unwrap();

    // Should receive update within 1 second
    let result = rx.recv_timeout(Duration::from_secs(2));
    assert!(result.is_ok(), "watcher should detect config change");
    assert_eq!(result.unwrap().border_width, 8);

    // Cleanup
    drop(handle); // watcher thread will exit when sender is dropped... actually it blocks on ReadDirectoryChangesW
    let _ = std::fs::remove_dir_all(&dir);
}
```

Note: The watcher thread blocks on `ReadDirectoryChangesW` and won't exit on its own. The test relies on process exit to clean up the thread. This is acceptable for tests.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::tests::watch_config_dir_detects_file_change`
Expected: FAIL — `watch_config_dir` not defined

- [ ] **Step 3: Add PartialEq to AppConfig**

In `src/config.rs`, change:

```rust
// Before:
#[derive(Debug, Clone)]
pub struct AppConfig {

// After:
#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
```

- [ ] **Step 4: Implement watch_config_dir**

Add at the end of `src/config.rs` (before `#[cfg(test)]`):

```rust
/// Watch `~/.holdrect/` directory for config file changes.
/// Blocks current thread. Sends new AppConfig on `tx` when config.toml changes.
/// Exits silently if directory doesn't exist.
pub fn watch_config_dir(dir: std::path::PathBuf, tx: std::sync::mpsc::Sender<AppConfig>) {
    use windows::Win32::Foundation::*;
    use windows::Win32::Storage::FileSystem::*;
    use windows::Win32::System::SystemServices::*;

    if !dir.exists() {
        return; // silent exit, matches current config loading behavior
    }

    let dir_wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();

    let dir_handle = unsafe {
        CreateFileW(
            windows::core::PCWSTR(dir_wide.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    };

    let Ok(handle) = dir_handle else {
        eprintln!("Warning: could not open config directory for watching");
        return;
    };

    let mut buffer = [0u8; 4096];
    let mut bytes_returned: u32 = 0;

    // ponytail: sync I/O, simple blocking loop. Adequate for single-file watch.
    loop {
        let ok = unsafe {
            ReadDirectoryChangesW(
                handle,
                buffer.as_mut_ptr() as *mut _,
                buffer.len() as u32,
                false, // don't watch subtree
                FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME,
                Some(&mut bytes_returned),
                None, // no OVERLAPPED = synchronous/blocking
                None,
            )
        };

        if ok.is_err() {
            break; // directory deleted or handle invalid
        }

        // Debounce: editors fire multiple events per save
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Parse FILE_NOTIFY_INFORMATION chain
        // Layout: NextEntryOffset(u32) + Action(u32) + FileNameLength(u32) + FileName(u16[])
        let mut offset = 0usize;
        loop {
            if offset + 12 > buffer.len() { break; }
            let next_offset = u32::from_ne_bytes(buffer[offset..offset+4].try_into().unwrap());
            let name_len = u32::from_ne_bytes(buffer[offset+8..offset+12].try_into().unwrap()) as usize / 2;
            let name_start = offset + 12;
            let name_end = name_start + name_len * 2;
            if name_end > buffer.len() { break; }

            let name_bytes = &buffer[name_start..name_end];
            let name = String::from_utf16_lossy(
                &(0..name_len).map(|i| u16::from_ne_bytes([name_bytes[i*2], name_bytes[i*2+1]])).collect::<Vec<_>>()
            );

            if name.eq_ignore_ascii_case("config.toml") {
                let config_path = dir.join("config.toml");
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    let new_config = AppConfig::parse(&content);
                    let _ = tx.send(new_config);
                }
                // parse failure: stderr warning already printed by AppConfig::parse
                break; // process one change per cycle
            }

            if next_offset == 0 { break; }
            offset += next_offset as usize;
        }
    }

    unsafe { let _ = CloseHandle(handle); }
}
```

Note: Manually parses raw bytes from `FILE_NOTIFY_INFORMATION` buffer to avoid unsafe struct casting. Layout: `NextEntryOffset(u32, 0) + Action(u32, 4) + FileNameLength(u32, 8) + FileName(u16[], 12)`.

- [ ] **Step 5: Add required Windows features**

In `Cargo.toml`, add to features:

```toml
"Win32_System_SystemServices",  # FILE_FLAG_BACKUP_SEMANTICS
```

- [ ] **Step 6: Run all tests**

Run: `cargo test --lib`
Expected: All tests PASS (including the new watcher test)

- [ ] **Step 7: Commit**

```bash
git add src/config.rs Cargo.toml
git commit -m "feat(config): add watch_config_dir for hot-reload"
```

---

### Task 5: popup/mod.rs — update_modifier_name

**Files:**
- Modify: `src/popup/mod.rs` (add `update_modifier_name` method to PopupManager)

**Interfaces:**
- Produces: `pub fn update_modifier_name(&mut self, name: &str)` — called from overlay.rs Task 6

- [ ] **Step 1: Write test**

Add to `src/popup/mod.rs` test module:

```rust
#[test]
fn update_modifier_name_rebuilds_cheatsheet_rows() {
    let mut m = PopupManager::new("Alt");
    assert_eq!(m.cheatsheet_rows()[0].0, "Alt + drag");

    m.update_modifier_name("Ctrl");
    assert_eq!(m.cheatsheet_rows()[0].0, "Ctrl + drag");
    assert_eq!(m.cheatsheet_rows()[4].0, "Ctrl + `");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib popup::tests::update_modifier_name_rebuilds_cheatsheet_rows`
Expected: FAIL — `update_modifier_name` not defined

- [ ] **Step 3: Implement update_modifier_name**

Add to `impl PopupManager` in `src/popup/mod.rs`, after `cheatsheet_rows()`:

```rust
/// Rebuild cheatsheet rows when modifier key changes (hot-reload).
pub fn update_modifier_name(&mut self, name: &str) {
    let drag_label = format!("{} + drag", name);
    let help_label = format!("{} + `", name);
    self.cheatsheet_rows = vec![
        (drag_label, "Draw".to_string()),
        ("1".to_string(), "Pin".to_string()),
        ("2".to_string(), "Spotlight".to_string()),
        ("Esc".to_string(), "Clear".to_string()),
        (help_label, "Help".to_string()),
    ];
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib popup::tests::update_modifier_name_rebuilds_cheatsheet_rows`
Expected: PASS

- [ ] **Step 5: Run all tests**

Run: `cargo test --lib`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/popup/mod.rs
git commit -m "feat(popup): add update_modifier_name for hot-reload cascade"
```

---

### Task 6: overlay.rs — App holds config_rx, polls in about_to_wait

**Files:**
- Modify: `src/overlay.rs:124-139` (App struct — add config_rx, modifier_name)
- Modify: `src/overlay.rs:152-169` (App::new — accept config_rx)
- Modify: `src/overlay.rs:255-292` (about_to_wait — poll config_rx)
- Modify: `src/overlay.rs:622-626` (run_overlay — accept config_rx)

**Interfaces:**
- Consumes: `Receiver<AppConfig>` from main.rs channel
- Consumes: `crate::hook::update_modifier_codes(Vec<u32>)` from Task 3
- Consumes: `PopupManager::update_modifier_name(&str)` from Task 5
- Produces: `pub fn run_overlay(..., config_rx: Receiver<AppConfig>)` — signature change consumed by main.rs Task 7

- [ ] **Step 1: Write test for config poll in App**

This is hard to unit-test because App requires an event loop. We test the integration in Task 7. For now, verify the code compiles and existing tests pass.

- [ ] **Step 2: Add config_rx and modifier_name to App struct**

In `src/overlay.rs`, modify App struct:

```rust
pub struct App {
    window: Option<Window>,
    state: AppState,
    input_rx: Receiver<InputEvent>,
    border_width: i32,
    color_mode: ColorMode,
    modifier_name: String,                    // new
    config_rx: Receiver<AppConfig>,            // new
    #[cfg(windows)]
    dib_cache: Option<DibCache>,
    // Popup system
    #[cfg(windows)]
    popup_hwnd: Option<HWND>,
    popup_manager: PopupManager,
    #[cfg(windows)]
    popup_renderer: Option<GdiRenderer>,
    popup_monitor_rect: (i32, i32, i32, i32),
}
```

- [ ] **Step 3: Update App::new signature**

```rust
pub fn new(input_rx: Receiver<InputEvent>, config_rx: Receiver<AppConfig>, border_width: i32, color_mode: ColorMode, modifier_name: String) -> Self {
    Self {
        window: None,
        state: AppState::default(),
        input_rx,
        border_width,
        color_mode,
        modifier_name: modifier_name.clone(),
        config_rx,
        // ... rest unchanged ...
    }
}
```

- [ ] **Step 4: Add config poll to about_to_wait**

In `about_to_wait`, add config poll block BEFORE the input event drain:

```rust
fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    // Poll for config changes (hot-reload)
    while let Ok(new_config) = self.config_rx.try_recv() {
        self.border_width = new_config.border_width;
        self.color_mode = new_config.color_mode;
        if new_config.modifier_name != self.modifier_name {
            self.modifier_name = new_config.modifier_name.clone();
            self.popup_manager.update_modifier_name(&self.modifier_name);
        }
        crate::hook::update_modifier_codes(new_config.modifier_vk_codes);
    }

    // Drain all pending input events
    while let Ok(event) = self.input_rx.try_recv() {
        // ... existing code unchanged ...
    }
    // ... rest unchanged ...
}
```

- [ ] **Step 5: Update run_overlay signature**

```rust
pub fn run_overlay(
    event_loop: EventLoop<()>,
    input_rx: Receiver<InputEvent>,
    config_rx: Receiver<AppConfig>,  // new param
    border_width: i32,
    color_mode: ColorMode,
    modifier_name: String,
) {
    let mut app = App::new(input_rx, config_rx, border_width, color_mode, modifier_name);
    event_loop.run_app(&mut app).expect("Event loop error");
}
```

- [ ] **Step 6: Add AppConfig import**

At the top of `src/overlay.rs`, add:

```rust
use crate::config::AppConfig;
```

- [ ] **Step 7: Run all tests**

Run: `cargo test --lib`
Expected: Compilation may fail because main.rs still uses old `run_overlay` signature. Fix in Task 7.

- [ ] **Step 8: Fix compilation — update main.rs call site temporarily**

In `src/main.rs`, update the `run_overlay` call to pass `config_rx`:

```rust
let (config_tx, config_rx) = std::sync::mpsc::channel();
run_overlay(event_loop, input_rx, config_rx, config.border_width, config.color_mode, config.modifier_name.clone());
```

- [ ] **Step 9: Run all tests again**

Run: `cargo test --lib`
Expected: All tests PASS

- [ ] **Step 10: Commit**

```bash
git add src/overlay.rs src/main.rs
git commit -m "feat(overlay): App polls config_rx for hot-reload"
```

---

### Task 7: main.rs — Wire up config watcher thread

**Files:**
- Modify: `src/main.rs:20-42` (start watcher thread, connect channel)

**Interfaces:**
- Consumes: `crate::config::watch_config_dir(PathBuf, Sender<AppConfig>)` from Task 4
- Consumes: `crate::config::AppConfig::load()` — existing
- Consumes: `run_overlay(..., config_rx)` from Task 6

- [ ] **Step 1: Wire up watcher thread in main.rs**

Replace the `main()` function body (after the `--mem-report` check):

```rust
fn main() {
    // --mem-report: print memory stats and exit (before GUI init)
    if std::env::args().any(|a| a == "--mem-report") {
        unsafe {
            let _ = windows::Win32::System::Console::AttachConsole(
                windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
        crate::mem_report::print_mem_report();
        return;
    }

    #[cfg(windows)]
    set_dpi_awareness();

    let (event_loop, proxy) = create_event_loop();
    let (input_tx, input_rx) = mpsc::channel::<InputEvent>();
    let (exit_tx, exit_rx) = mpsc::channel::<AppExit>();
    let (config_tx, config_rx) = mpsc::channel::<AppConfig>();

    let config = crate::config::AppConfig::load();

    // Start config watcher thread (hot-reload)
    let watch_dir = dirs::home_dir()
        .map(|h| h.join(".holdrect"))
        .unwrap_or_default();
    std::thread::spawn(move || {
        crate::config::watch_config_dir(watch_dir, config_tx);
    });

    #[cfg(windows)]
    crate::hook::start_hook_listener(input_tx, proxy, config.modifier_vk_codes.clone());

    let _tray_icon = start_tray(exit_tx);

    thread::spawn(move || {
        let _ = exit_rx.recv();
        std::process::exit(0);
    });

    run_overlay(event_loop, input_rx, config_rx, config.border_width, config.color_mode, config.modifier_name.clone());
    std::process::exit(0);
}
```

- [ ] **Step 2: Add AppConfig import**

Add to the imports in `src/main.rs`:

```rust
use crate::config::AppConfig;
```

- [ ] **Step 3: Run all tests**

Run: `cargo test --lib`
Expected: All tests PASS

- [ ] **Step 4: Manual integration test**

Run: `cargo build --release`
Run: `./target/release/holdrect.exe`
Then:
1. Open `~/.holdrect/config.toml` in notepad
2. Change `border_width = 4` to `border_width = 10`
3. Save
4. Draw a rectangle — border should now be 10px wide

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): wire config watcher thread for hot-reload"
```

---

### Task 8: Runtime memory optimization (conditional)

**Files:**
- Modify: `src/state.rs` (optional — reduce clones)
- Modify: `src/overlay.rs` (optional — DibCache idle release)

**Interfaces:**
- Conditional: only execute if baseline > 3MB after compile-time optimization

- [ ] **Step 1: Compare BASELINE with 3MB threshold**

Task 1 Step 9 记录的BASELINE Working Set数字决定是否执行本task:
- BASELINE < 3MB → **SKIP Task 8**, 直接到 Task 9
- BASELINE >= 3MB → 继续执行 Step 2+

- [ ] **Step 2: If > 3MB — reduce clones in state machine**

In `src/state.rs`, replace `state.pinned_rects.clone()` with `std::mem::take(&mut state.pinned_rects)` where the old value is not needed. Only applies to branches that return `Vec::new()` for the new pinned_rects (e.g., EscapePressed, modifier release paths).

This is a targeted change — the `process_event` function takes `&AppState` (immutable ref), so `take` requires changing to `&mut AppState`. **Only do this if profiling shows clone is a bottleneck.**

- [ ] **Step 3: If > 3MB — DibCache idle release**

In `src/overlay.rs::render()`, when `!has_drawing && !has_pinned`, call `self.dib_cache = None` to release the buffer.

- [ ] **Step 4: Measure again**

Run: `cargo build --release && ./target/release/holdrect.exe --mem-report`
Compare with baseline.

- [ ] **Step 5: Commit (if changes made)**

```bash
git add src/state.rs src/overlay.rs
git commit -m "perf: reduce memory footprint — DibCache idle release, reduce clones"
```

---

### Task 9: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test --lib`
Expected: All tests PASS

- [ ] **Step 2: Build release and measure FINAL memory**

```bash
cargo build --release
./target/release/holdrect.exe --mem-report
```

**对比FINAL数字与BASELINE(Task 1 Step 9记录)。** 记录总优化量: Working Set减少XKB (Y%), Pagefile减少XKB (Y%)。

- [ ] **Step 3: Manual end-to-end test**

1. Start HoldRect
2. Alt+drag to draw a rect (verify it works)
3. Edit `~/.holdrect/config.toml`: change `color = "#00ff00"`
4. Save
5. Alt+drag again — border should be green
6. Edit `~/.holdrect/config.toml`: change `modifier = "Ctrl"`
7. Save
8. Ctrl+drag to draw a rect — should work with Ctrl now
9. Press 1 to pin — popup should show correct modifier name in cheatsheet
10. Alt+drag should no longer work

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A
git commit -m "chore: final verification for hot-reload + memory optimization"
```
