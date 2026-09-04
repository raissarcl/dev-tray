//! Ensures only one Dev Tray process is alive.
//!
//! A second launch acquires the same named mutex, sees it already exists, and
//! exits immediately (no extra tray icon, proxy, or hosts writes).

#[cfg(windows)]
const MUTEX_NAME: windows::core::PCWSTR =
    windows::core::w!("Local\\com.rairc.dev-tray.single-instance");

/// Returns `true` when this process should continue as the active instance.
///
/// The mutex handle is left open on purpose so the OS holds the lock until exit.
/// Mutex creation failures do not block startup.
pub fn acquire() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;

        unsafe {
            match CreateMutexW(None, false, MUTEX_NAME) {
                Ok(handle) => {
                    if GetLastError() == ERROR_ALREADY_EXISTS {
                        let _ = CloseHandle(handle);
                        eprintln!("[dev-tray] already running — ignoring this launch");
                        false
                    } else {
                        true
                    }
                }
                Err(err) => {
                    eprintln!("[dev-tray] single-instance lock failed ({err}); continuing");
                    true
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        true
    }
}
