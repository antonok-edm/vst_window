//! Platform-specific utilities for Windows.

mod event_source;
mod window;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use winapi::um::errhandlingapi;

use crate::SetupError;

use self::{event_source::EventSource, window::ChildWindow};

use super::EditorWindowBackend;

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
    (error, format!("error code {}", error))
}

fn format_last_error(called_fn: &'static str) -> String {
    format!("call to {} failed: {}", called_fn, get_last_error().1)
}

fn wrap_last_error(called_fn: &'static str) -> SetupError {
    SetupError::new_boxed(format_last_error(called_fn).into())
}

pub struct EditorWindowImpl {
    event_source: EventSource, // drop first
    window: ChildWindow,       // drop second
}

unsafe impl HasRawWindowHandle for EditorWindowImpl {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.window.raw_window_handle()
    }
}

impl EditorWindowBackend for EditorWindowImpl {
    unsafe fn build(
        parent: *mut std::os::raw::c_void,
        size_xy: (i32, i32),
    ) -> Result<Self, SetupError> {
        let window = unsafe { ChildWindow::build(parent, size_xy)? };
        let event_source = EventSource::new(&window, size_xy)?;

        Ok(EditorWindowImpl {
            event_source,
            window,
        })
    }

    fn poll_event(&self) -> Option<crate::WindowEvent> {
        self.event_source.poll_event()
    }
}
