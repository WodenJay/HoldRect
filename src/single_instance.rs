//! Single instance enforcement using Windows named mutex.
//!
//! Ensures only one instance of HoldRect can run at a time.

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::MessageBoxW;

/// Result of single instance check.
pub enum SingleInstance {
    /// This is the first instance. The mutex handle must be kept alive.
    First(HANDLE),
    /// Another instance is already running.
    AlreadyRunning,
}

/// Attempts to acquire the single-instance mutex.
///
/// Returns `SingleInstance::First(handle)` if this is the first instance,
/// or `SingleInstance::AlreadyRunning` if another instance is running.
///
/// **Important**: The returned HANDLE must be kept alive for the entire
/// program lifetime, otherwise the mutex will be released.
pub fn try_acquire() -> SingleInstance {
    unsafe {
        let mutex_name: Vec<u16> = "Global\\HoldRect_SingleInstance\0".encode_utf16().collect();
        let handle = CreateMutexW(None, false, windows::core::PCWSTR(mutex_name.as_ptr()));
        match handle {
            Ok(h) => {
                // Check if this mutex already existed
                // GetLastError returns ERROR_ALREADY_EXISTS (183) if mutex existed
                let last_error = windows::Win32::Foundation::GetLastError();
                if last_error == windows::Win32::Foundation::WIN32_ERROR(183) {
                    // Mutex already existed - another instance is running
                    SingleInstance::AlreadyRunning
                } else {
                    // Successfully created new mutex - this is the first instance
                    SingleInstance::First(h)
                }
            }
            Err(e) => {
                // ERROR_ALREADY_EXISTS (183) means another instance is running
                if e.code().0 == 183 {
                    SingleInstance::AlreadyRunning
                } else {
                    // Other error - treat as first instance
                    SingleInstance::First(HANDLE::default())
                }
            }
        }
    }
}

/// Shows a message box with the given text.
fn show_message(text: &str) {
    unsafe {
        let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let title_w: Vec<u16> = "HoldRect\0".encode_utf16().collect();
        MessageBoxW(
            None,
            windows::core::PCWSTR(text_w.as_ptr()),
            windows::core::PCWSTR(title_w.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::MB_OK
                | windows::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION,
        );
    }
}

/// Shows the "already started" message and exits.
pub fn show_already_running_and_exit() -> ! {
    show_message("HoldRect is already running.\n\nLook for the tray icon in the system tray.");
    std::process::exit(0);
}

/// Shows the "started" message for the first instance.
pub fn show_started() {
    show_message("HoldRect has started.\n\nUse Alt+Left-click drag to draw rectangles.\nLook for the tray icon in the system tray.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::CloseHandle;

    /// Test that the first call to try_acquire returns First.
    /// This test uses a unique mutex name to avoid conflicts with other tests
    /// or the actual running application.
    #[test]
    fn first_call_returns_first() {
        // Use a test-specific mutex name
        let result = test_try_acquire_with_name("Global\\HoldRect_TestMutex_FirstCall");
        match result {
            SingleInstance::First(handle) => {
                // Clean up the mutex handle
                if !handle.is_invalid() {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                }
            }
            SingleInstance::AlreadyRunning => {
                panic!("First call should return First, not AlreadyRunning");
            }
        }
    }

    /// Test that a second call in the same process returns AlreadyRunning.
    #[test]
    fn second_call_returns_already_running() {
        let mutex_name = "Global\\HoldRect_TestMutex_SecondCall";
        let first_result = test_try_acquire_with_name(mutex_name);

        match first_result {
            SingleInstance::First(handle) => {
                // Keep handle alive during second call
                let second_result = test_try_acquire_with_name(mutex_name);
                match second_result {
                    SingleInstance::AlreadyRunning => {
                        // Expected behavior - second call detected existing mutex
                    }
                    SingleInstance::First(_) => {
                        panic!("Second call should return AlreadyRunning when mutex exists");
                    }
                }
                // Clean up
                if !handle.is_invalid() {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                }
            }
            SingleInstance::AlreadyRunning => {
                panic!("First call unexpectedly returned AlreadyRunning");
            }
        }
    }

    /// Test that different mutex names are independent.
    #[test]
    fn different_mutex_names_independent() {
        let result1 = test_try_acquire_with_name("Global\\HoldRect_TestMutex_Independent1");
        let result2 = test_try_acquire_with_name("Global\\HoldRect_TestMutex_Independent2");

        // Both should be First since they use different names
        let handle1 = match result1 {
            SingleInstance::First(h) => h,
            SingleInstance::AlreadyRunning => {
                panic!("First mutex should return First");
            }
        };

        let handle2 = match result2 {
            SingleInstance::First(h) => h,
            SingleInstance::AlreadyRunning => {
                panic!("Second mutex with different name should return First");
            }
        };

        // Clean up
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

    /// Test that mutex is released when handle is closed.
    #[test]
    fn mutex_released_after_handle_close() {
        let mutex_name = "Global\\HoldRect_TestMutex_ReleaseAfterClose";

        // Acquire and release
        let handle = match test_try_acquire_with_name(mutex_name) {
            SingleInstance::First(h) => h,
            SingleInstance::AlreadyRunning => {
                panic!("First acquire should succeed");
            }
        };

        // Close the handle to release the mutex
        if !handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(handle);
            }
        }

        // Small delay to ensure mutex is released
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Should be able to acquire again
        let result = test_try_acquire_with_name(mutex_name);
        let handle2 = match result {
            SingleInstance::First(h) => h,
            SingleInstance::AlreadyRunning => {
                panic!("Should be able to acquire after handle closed");
            }
        };

        // Clean up
        if !handle2.is_invalid() {
            unsafe {
                let _ = CloseHandle(handle2);
            }
        }
    }

    /// Helper function to test mutex acquisition with a custom name.
    fn test_try_acquire_with_name(name: &str) -> SingleInstance {
        unsafe {
            let mutex_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            let handle = CreateMutexW(
                None,
                false,
                windows::core::PCWSTR(mutex_name.as_ptr()),
            );
            match handle {
                Ok(h) => {
                    let last_error = windows::Win32::Foundation::GetLastError();
                    if last_error == windows::Win32::Foundation::WIN32_ERROR(183) {
                        SingleInstance::AlreadyRunning
                    } else {
                        SingleInstance::First(h)
                    }
                }
                Err(e) => {
                    if e.code().0 == 183 {
                        SingleInstance::AlreadyRunning
                    } else {
                        SingleInstance::First(HANDLE::default())
                    }
                }
            }
        }
    }
}
