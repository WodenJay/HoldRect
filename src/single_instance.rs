//! Single instance enforcement using Windows named mutex.
//!
//! Ensures only one instance of HoldRect can run at a time.
//! Uses `FindWindow` + `PostMessage` to notify the existing instance
//! via a registered custom Windows message.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::CreateMutexW;

/// Custom Windows message name for single-instance notification.
/// Both the main instance (overlay) and second instance register this
/// via `RegisterWindowMessageW` to get the same message ID.
pub const ALREADY_RUNNING_MSG_NAME: &str = "HoldRect_AlreadyRunning";

/// Result of single instance check.
pub enum SingleInstance {
    /// This is the first instance. The mutex handle must be kept alive.
    First(HANDLE),
    /// Another instance is already running.
    AlreadyRunning,
}

/// Attempts to acquire the single-instance mutex.
///
/// Returns `Ok(First(handle))` if this is the first instance,
/// `Ok(AlreadyRunning)` if another instance is running,
/// or `Err` if mutex creation failed for an unexpected reason.
///
/// **Important**: The returned HANDLE must be kept alive for the entire
/// program lifetime, otherwise the mutex will be released.
pub fn try_acquire() -> Result<SingleInstance, windows::core::Error> {
    unsafe {
        let mutex_name: Vec<u16> = "Global\\HoldRect_SingleInstance\0".encode_utf16().collect();
        let handle = CreateMutexW(None, false, windows::core::PCWSTR(mutex_name.as_ptr()))?;
        // GetLastError returns ERROR_ALREADY_EXISTS (183) if mutex already existed
        let last_error = windows::Win32::Foundation::GetLastError();
        if last_error == windows::Win32::Foundation::WIN32_ERROR(183) {
            Ok(SingleInstance::AlreadyRunning)
        } else {
            Ok(SingleInstance::First(handle))
        }
    }
}

/// Notify the existing HoldRect instance that a second instance tried to start.
///
/// Uses `FindWindow` to locate the popup window by its class name `"HoldRectPopup"`,
/// then posts a custom registered message (same name the main instance registered)
/// to trigger the "Already running" slide-in popup.
///
/// Silently exits if the window can't be found (main instance may have just closed).
pub fn notify_existing_instance() {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, PostMessageW, RegisterWindowMessageW,
    };

    unsafe {
        let msg_name: Vec<u16> = ALREADY_RUNNING_MSG_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let msg_id = RegisterWindowMessageW(windows::core::PCWSTR(msg_name.as_ptr()));
        if msg_id == 0 {
            return;
        }

        let class_name: Vec<u16> = "HoldRectPopup\0".encode_utf16().collect();
        let hwnd = match FindWindowW(
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR(std::ptr::null()),
        ) {
            Ok(h) => h,
            Err(_) => return,
        };
        if hwnd.is_invalid() || hwnd == Default::default() {
            return;
        }

        let _ = PostMessageW(hwnd, msg_id, WPARAM(0), LPARAM(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::CloseHandle;

    #[test]
    fn first_call_returns_first() {
        let result = test_try_acquire_with_name("Global\\HoldRect_TestMutex_FirstCall");
        match result {
            Ok(SingleInstance::First(handle)) => {
                if !handle.is_invalid() {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                }
            }
            Ok(SingleInstance::AlreadyRunning) => {
                panic!("First call should return Ok(First), not Ok(AlreadyRunning)");
            }
            Err(e) => {
                panic!("First call should return Ok(First), got Err: {:?}", e);
            }
        }
    }

    #[test]
    fn second_call_returns_already_running() {
        let mutex_name = "Global\\HoldRect_TestMutex_SecondCall";
        let first_result = test_try_acquire_with_name(mutex_name);

        let handle = match first_result {
            Ok(SingleInstance::First(h)) => h,
            Ok(SingleInstance::AlreadyRunning) => {
                panic!("First call unexpectedly returned AlreadyRunning");
            }
            Err(e) => {
                panic!("First call failed: {:?}", e);
            }
        };

        let second_result = test_try_acquire_with_name(mutex_name);
        match second_result {
            Ok(SingleInstance::AlreadyRunning) => {}
            Ok(SingleInstance::First(_)) => {
                panic!("Second call should return AlreadyRunning");
            }
            Err(e) => {
                panic!("Second call failed: {:?}", e);
            }
        }

        if !handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
    }

    #[test]
    fn different_mutex_names_independent() {
        let result1 = test_try_acquire_with_name("Global\\HoldRect_TestMutex_Ind1");
        let result2 = test_try_acquire_with_name("Global\\HoldRect_TestMutex_Ind2");

        let handle1 = match result1 {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("First mutex should return Ok(First)"),
        };
        let handle2 = match result2 {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("Second mutex should return Ok(First)"),
        };

        if !handle1.is_invalid() {
            unsafe {
                let _ = CloseHandle(handle1);
            }
        }
        if !handle2.is_invalid() {
            unsafe {
                let _ = CloseHandle(handle2);
            }
        }
    }

    #[test]
    fn mutex_released_after_handle_close() {
        let mutex_name = "Global\\HoldRect_TestMutex_Release";

        let handle = match test_try_acquire_with_name(mutex_name) {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("First acquire should succeed"),
        };

        if !handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));

        let handle2 = match test_try_acquire_with_name(mutex_name) {
            Ok(SingleInstance::First(h)) => h,
            _ => panic!("Should acquire after handle closed"),
        };

        if !handle2.is_invalid() {
            unsafe {
                let _ = CloseHandle(handle2);
            }
        }
    }

    fn test_try_acquire_with_name(name: &str) -> Result<SingleInstance, windows::core::Error> {
        unsafe {
            let mutex_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = CreateMutexW(None, false, windows::core::PCWSTR(mutex_name.as_ptr()))?;
            let last_error = windows::Win32::Foundation::GetLastError();
            if last_error == windows::Win32::Foundation::WIN32_ERROR(183) {
                Ok(SingleInstance::AlreadyRunning)
            } else {
                Ok(SingleInstance::First(handle))
            }
        }
    }
}
