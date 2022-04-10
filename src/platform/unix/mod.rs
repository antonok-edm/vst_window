//! Platform-specific utilities for Unix.

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::SetupError;

use self::{event_source::EventSource, window::ChildWindow};

use super::EditorWindowBackend;

mod error;
mod event_source;
mod window;

pub struct EditorWindowImpl {
    event_source: EventSource,
    window: ChildWindow,
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
        let window = ChildWindow::build(parent, size_xy)?;
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
