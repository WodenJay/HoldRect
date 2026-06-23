# 开机自启 Tray Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "开机自启" checkbox to the system tray menu that toggles Windows Registry auto-start.

**Architecture:** New `src/autostart.rs` module with pure registry read/write functions. `src/tray.rs` modified to add a `CheckMenuItem` that calls these functions. No changes to config, overlay, hook, or state modules.

**Tech Stack:** `windows` crate (`Win32_System_Registry`), `tray-icon` crate (`CheckMenuItem` from `muda` re-export)

## Global Constraints

- `#[cfg(windows)]` gate all autostart code
- Registry key: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value name `"HoldRect"`
- `is_autostart_enabled()` checks value name existence only (not path match)
- `set_autostart(false)` treats `ERROR_FILE_NOT_FOUND` as success
- All opened registry handles must be closed
- Operation is idempotent — concurrent clicks are safe
- Tests mock-free, operate on real HKCU registry

---

### Task 1: Add `Win32_System_Registry` feature to Cargo.toml

**Files:**
- Modify: `Cargo.toml:15` — add feature to `windows` crate

**Interfaces:**
- Produces: `windows::Win32::System::Registry` API available for Task 2

- [ ] **Step 1: Add the feature**

```toml
# Cargo.toml, line 15 — add "Win32_System_Registry" to the features array:
windows = { version = "0.58", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_Foundation",
    "Win32_UI_HiDpi",
    "Win32_Graphics_Gdi",
    "Win32_System_Registry",
] }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add Win32_System_Registry feature to windows crate"
```

---

### Task 2: Create `src/autostart.rs` — registry functions + tests (TDD)

**Files:**
- Create: `src/autostart.rs`
- Modify: `src/main.rs:3` — add `mod autostart;`

**Interfaces:**
- Produces: `pub fn is_autostart_enabled() -> bool` — consumed by Task 3
- Produces: `pub fn set_autostart(enable: bool) -> Result<(), Box<dyn std::error::Error>>` — consumed by Task 3

- [ ] **Step 1: Create `src/autostart.rs` with failing tests first**

```rust
// src/autostart.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_then_check() {
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled(), "should be enabled after set(true)");
    }

    #[test]
    fn disable_then_check() {
        set_autostart(true).unwrap();
        set_autostart(false).unwrap();
        assert!(!is_autostart_enabled(), "should be disabled after set(false)");
    }

    #[test]
    fn enable_idempotent() {
        set_autostart(true).unwrap();
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled(), "double enable should still be enabled");
    }

    #[test]
    fn disable_idempotent() {
        set_autostart(false).unwrap();
        set_autostart(false).unwrap();
        assert!(!is_autostart_enabled(), "double disable should still be disabled");
    }

    #[test]
    fn exe_path_quoted_with_spaces() {
        let path = quoted_exe_path();
        assert!(path.starts_with('"'), "path should start with quote: {}", path);
        assert!(path.ends_with('"'), "path should end with quote: {}", path);
    }

    #[test]
    fn stale_path_overwritten_on_enable() {
        // Write a stale path
        set_autostart(true).unwrap();
        // Enable again — should overwrite with current path (no error)
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled());
    }

    #[test]
    fn disable_nonexistent_value_succeeds() {
        set_autostart(false).unwrap();
        set_autostart(false).unwrap(); // already absent
        // Should not panic or return error
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test autostart --quiet 2>&1`
Expected: compilation errors — `is_autostart_enabled`, `set_autostart`, `quoted_exe_path` not found

- [ ] **Step 3: Register module in `src/main.rs`**

Add after line 2 (`mod config;`):
```rust
#[cfg(windows)]
mod autostart;
```

- [ ] **Step 4: Implement `src/autostart.rs`**

> **API notes (from review):**
> - All `Reg*W` functions return `WIN32_ERROR`, NOT `Result`. Use `.ok()?` to propagate errors.
> - `RegOpenKeyExW` 3rd param (`uloptions`) is `u32`, pass `0u32`.
> - `RegSetValueExW` 3rd param (`reserved`) is `u32`, pass `0u32`.
> - `RegSetValueExW` 5th param (`lpdata`) is `Option<&[u8]>`, pass wide bytes directly.
> - `RegDeleteValueW` returns `WIN32_ERROR`; use `.ok()` then match to handle `ERROR_FILE_NOT_FOUND`.
> - Always close `HKEY` handle before returning (closure pattern for error safety).

```rust
// src/autostart.rs
use std::ffi::OsStr;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::*;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "HoldRect";

/// Check if "HoldRect" value exists in the Run key (path-agnostic).
fn quoted_exe_path() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let path = exe.to_string_lossy();
    format!("\"{}\"", path)
}

fn run_key() -> Result<HKEY, windows::core::Error> {
    let mut hkey = HKEY::default();
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, 0u32, KEY_READ | KEY_WRITE, &mut hkey).ok()?;
    }
    Ok(hkey)
}

pub fn is_autostart_enabled() -> bool {
    let hkey = match run_key() {
        Ok(h) => h,
        Err(_) => return false,
    };
    let result = unsafe { RegQueryValueExW(hkey, VALUE_NAME, None, None, None, None) };
    unsafe { let _ = RegCloseKey(hkey); }
    result.is_ok()
}

pub fn set_autostart(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let hkey = run_key()?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if enable {
            let value = quoted_exe_path();
            let wide: Vec<u16> = OsStr::new(&value)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wide_bytes = unsafe {
                std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2)
            };
            unsafe { RegSetValueExW(hkey, VALUE_NAME, 0u32, REG_SZ, Some(wide_bytes)).ok()?; }
        } else {
            let del_result = unsafe { RegDeleteValueW(hkey, VALUE_NAME) }.ok();
            match del_result {
                Ok(()) => {}
                Err(e) if e.code() == ERROR_FILE_NOT_FOUND.into() => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    })();
    unsafe { let _ = RegCloseKey(hkey); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_then_check() {
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled(), "should be enabled after set(true)");
    }

    #[test]
    fn disable_then_check() {
        set_autostart(true).unwrap();
        set_autostart(false).unwrap();
        assert!(!is_autostart_enabled(), "should be disabled after set(false)");
    }

    #[test]
    fn enable_idempotent() {
        set_autostart(true).unwrap();
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled(), "double enable should still be enabled");
    }

    #[test]
    fn disable_idempotent() {
        set_autostart(false).unwrap();
        set_autostart(false).unwrap();
        assert!(!is_autostart_enabled(), "double disable should still be disabled");
    }

    #[test]
    fn exe_path_quoted_with_spaces() {
        let path = quoted_exe_path();
        assert!(path.starts_with('"'), "path should start with quote: {}", path);
        assert!(path.ends_with('"'), "path should end with quote: {}", path);
    }

    #[test]
    fn stale_path_overwritten_on_enable() {
        set_autostart(true).unwrap();
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled());
    }

    #[test]
    fn disable_nonexistent_value_succeeds() {
        set_autostart(false).unwrap();
        set_autostart(false).unwrap();
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test autostart --quiet 2>&1`
Expected: 7 passed, 0 failed

- [ ] **Step 6: Commit**

```bash
git add src/autostart.rs src/main.rs
git commit -m "feat(autostart): registry read/write for Windows auto-start"
```

---

### Task 3: Add `CheckMenuItem` to tray menu (TDD)

**Files:**
- Modify: `src/tray.rs` — add autostart toggle to menu

**Interfaces:**
- Consumes: `is_autostart_enabled()` from `src/autostart.rs`
- Consumes: `set_autostart(bool)` from `src/autostart.rs`

- [ ] **Step 1: Add failing tests to `src/tray.rs`**

Append to `src/tray.rs` tests module:

```rust
    #[test]
    fn check_menu_item_import_works() {
        use tray_icon::menu::CheckMenuItem;
        let item = CheckMenuItem::new("Test", true, false, None);
        assert!(!item.is_checked());
    }

    #[test]
    fn autostart_initial_state_reflects_registry() {
        use crate::autostart::{is_autostart_enabled, set_autostart};
        // Ensure disabled first
        set_autostart(false).unwrap();
        let state = is_autostart_enabled();
        assert!(!state);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tray --quiet 2>&1`
Expected: first test may pass (CheckMenuItem exists), second passes — compile to verify imports work

- [ ] **Step 3: Update `src/tray.rs` menu construction**

Replace the `start_tray` function body:

```rust
use std::sync::mpsc::Sender;

use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

use crate::autostart::{is_autostart_enabled, set_autostart};

/// Application exit signal type
#[derive(Clone, Debug, PartialEq)]
pub struct AppExit;

/// Create system tray icon with autostart toggle and quit menu.
/// Returns the TrayIcon (must be kept alive) and sends AppExit on quit.
pub fn start_tray(exit_tx: Sender<AppExit>) -> TrayIcon {
    let autostart_item = CheckMenuItem::new("开机自启", true, is_autostart_enabled(), None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("退出 HoldRect", true, None);

    let tray_menu = Menu::new();
    tray_menu.append(&autostart_item).expect("Failed to add autostart item");
    tray_menu.append(&separator).expect("Failed to add separator");
    tray_menu.append(&quit_item).expect("Failed to add quit item");

    let icon = create_icon();

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("HoldRect - 按住Ctrl+拖拽画框")
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");

    let quit_id = quit_item.id().clone();
    let autostart_id = autostart_item.id().clone();
    std::thread::spawn(move || loop {
        if let Ok(event) = MenuEvent::receiver().recv() {
            if event.id == quit_id {
                let _ = exit_tx.send(AppExit);
                break;
            } else if event.id == autostart_id {
                let new_state = !is_autostart_enabled();
                let _ = set_autostart(new_state);
                autostart_item.set_checked(new_state);
            }
        }
    });

    tray_icon
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test --quiet 2>&1`
Expected: all pass (203 + autostart + new tray tests)

- [ ] **Step 5: Commit**

```bash
git add src/tray.rs
git commit -m "feat(tray): add autostart toggle checkbox to system tray menu"
```

---

### Task 4: Full verification + cleanup

- [ ] **Step 1: Run full test suite**

Run: `cargo test --quiet 2>&1`
Expected: all pass, 0 failures

- [ ] **Step 2: Run `cargo build --release`**

Run: `cargo build --release 2>&1`
Expected: builds successfully

- [ ] **Step 3: Manual verification**

Run the exe:
1. Right-click tray icon → "开机自启" should be unchecked
2. Click it → should become checked, registry `HKCU\...\Run\HoldRect` exists
3. Click again → unchecked, registry value removed
4. Right-click tray → "退出 HoldRect" still works

- [ ] **Step 4: Commit if any fixes needed**

```bash
git commit -m "fix: <describe>"
```
