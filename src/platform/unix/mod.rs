//! Platform-specific utilities for Unix.

use anyhow::Context;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use self::{event_source::EventSource, window::ChildWindow};

use super::EditorWindowBackend;

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
    unsafe fn build(parent: *mut std::os::raw::c_void, size_xy: (i32, i32))
        -> anyhow::Result<Self> {
        let window = ChildWindow::build(parent, size_xy).context("couldn't initialize child window")?;
        let event_source = EventSource::new(&window, size_xy).context("couldn't initialize event handler")?;

        Ok(EditorWindowImpl {
            event_source,
            window,
        })
    }

    fn poll_event(&self) -> Option<crate::WindowEvent> {
        self.event_source.poll_event()
    }
}
