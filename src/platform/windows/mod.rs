//! Platform-specific utilities for Windows.

pub mod event_source;
pub mod window;

use winapi::um::errhandlingapi;

#[cfg(feature = "windows-error")]
fn get_last_error() -> (u32, String) {
    let error = unsafe { errhandlingapi::GetLastError() };
    (
        error,
        format!("{} ({})", windows_error::format_error(error), error),
    )
}

#[cfg(not(feature = "windows-error"))]
fn get_last_error() -> (u32, String) {
    let error = unsafe { errhandlingapi::GetLastError() };
    (error, format!("win32 error number: {}", error))
}
