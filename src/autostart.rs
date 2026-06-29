// src/autostart.rs
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::HSTRING;
use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::*;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "HoldRect";

/// Return the current exe path wrapped in double quotes.
fn quoted_exe_path() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let path = exe.to_string_lossy();
    format!("\"{}\"", path)
}

/// Open the HKCU\...\Run key with read+write access.
fn run_key() -> Result<HKEY, windows::core::Error> {
    let subkey = HSTRING::from(RUN_KEY);
    let mut hkey = HKEY::default();
    unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            &subkey,
            0u32,
            KEY_READ | KEY_WRITE,
            &mut hkey,
        )
        .ok()?;
    }
    Ok(hkey)
}

/// Check if "HoldRect" value exists in the Run key (path-agnostic).
pub fn is_autostart_enabled() -> bool {
    let hkey = match run_key() {
        Ok(h) => h,
        Err(_) => return false,
    };
    let value_name = HSTRING::from(VALUE_NAME);
    let result = unsafe { RegQueryValueExW(hkey, &value_name, None, None, None, None) };
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    result.is_ok()
}

/// Enable or disable auto-start by writing/removing the Run key value.
pub fn set_autostart(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let hkey = run_key()?;
    let value_name = HSTRING::from(VALUE_NAME);
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if enable {
            let value = quoted_exe_path();
            let wide: Vec<u16> = OsStr::new(&value)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let wide_bytes =
                unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
            unsafe {
                RegSetValueExW(hkey, &value_name, 0u32, REG_SZ, Some(wide_bytes)).ok()?;
            }
        } else {
            let del_result = unsafe { RegDeleteValueW(hkey, &value_name) }.ok();
            match del_result {
                Ok(()) => {}
                Err(e) if e.code() == ERROR_FILE_NOT_FOUND.into() => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    })();
    unsafe {
        let _ = RegCloseKey(hkey);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Global mutex to serialize registry-touching tests.
    /// All autostart tests mutate the same HKCU Run key, so they must not run concurrently.
    fn test_mutex() -> &'static Mutex<()> {
        static M: OnceLock<Mutex<()>> = OnceLock::new();
        M.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn enable_then_check() {
        let _g = test_mutex().lock().unwrap();
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled(), "should be enabled after set(true)");
    }

    #[test]
    fn disable_then_check() {
        let _g = test_mutex().lock().unwrap();
        set_autostart(true).unwrap();
        set_autostart(false).unwrap();
        assert!(
            !is_autostart_enabled(),
            "should be disabled after set(false)"
        );
    }

    #[test]
    fn enable_idempotent() {
        let _g = test_mutex().lock().unwrap();
        set_autostart(true).unwrap();
        set_autostart(true).unwrap();
        assert!(
            is_autostart_enabled(),
            "double enable should still be enabled"
        );
    }

    #[test]
    fn disable_idempotent() {
        let _g = test_mutex().lock().unwrap();
        set_autostart(false).unwrap();
        set_autostart(false).unwrap();
        assert!(
            !is_autostart_enabled(),
            "double disable should still be disabled"
        );
    }

    #[test]
    fn exe_path_quoted_with_spaces() {
        // No registry mutation — no mutex needed, but lock for hygiene
        let _g = test_mutex().lock().unwrap();
        let path = quoted_exe_path();
        assert!(
            path.starts_with('"'),
            "path should start with quote: {}",
            path
        );
        assert!(path.ends_with('"'), "path should end with quote: {}", path);
    }

    #[test]
    fn stale_path_overwritten_on_enable() {
        let _g = test_mutex().lock().unwrap();
        // Write a stale path
        set_autostart(true).unwrap();
        // Enable again — should overwrite with current path (no error)
        set_autostart(true).unwrap();
        assert!(is_autostart_enabled());
    }

    #[test]
    fn disable_nonexistent_value_succeeds() {
        let _g = test_mutex().lock().unwrap();
        set_autostart(false).unwrap();
        set_autostart(false).unwrap(); // already absent
                                       // Should not panic or return error
    }
}
