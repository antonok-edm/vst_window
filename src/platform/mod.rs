//! Exposes platform-specific application logic using a cross-platform API.
//!
//! Each platform-specific implementation is done within a correspondingly named module (`unix`,
//! `macos`, `windows`). Each platform module has two submodules - `window` and `event_source`.
//!
//! The platform-specific `window` module exposes an `EditorWindowImpl` type that implements
//! `EditorWindowBackend`.
//!
//! The platform-specific `event_source` module exposes an `EventSourceImpl` type that implements
//! `EventSourceBackend`.
//!
//! This module contains wrapper code to alias the particular platform-specific module as `os`, and
//! expose it under more the more restrictive `EditorWindow` and `EventSource` public types.

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::event::WindowEvent;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod unix;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use unix as os;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as os;

use os::event_source::EventSourceImpl;
use os::window::EditorWindowImpl;

/// Crate-internal cross-platform window handle creation API required on each platform.
trait EditorWindowBackend: raw_window_handle::HasRawWindowHandle {
    /// Builds a platform-specific window, using a provided window handle as a parent window.
    fn build(parent: *mut std::os::raw::c_void, size_xy: (i32, i32)) -> Self;
}

/// Crate-internal cross-platform event source API required on each platform.
trait EventSourceBackend {
    /// Builds a platform-specific event source corresponding to the provided window.
    fn new(window: &EditorWindowImpl, size_xy: (i32, i32)) -> Self;
    /// Returns the next `WindowEvent`, if one is available.
    fn poll_event(&self) -> Option<WindowEvent>;
}

/// Build a platform-specific window and return a cross-platform `RawWindowHandle` implementor,
/// used as a surface for rendering, as well as a cross-platform `EventSource`, which is used to
/// poll `WindowEvent`s.
///
/// `parent` should be a window handle as passed from a host to a plugin by the `vst` crate.
pub fn setup(
    parent: *mut std::os::raw::c_void,
    size_xy: (i32, i32),
) -> (EditorWindow, EventSource) {
    let window = EditorWindowImpl::build(parent, size_xy);
    let event_source = EventSourceImpl::new(&window, size_xy);
    (EditorWindow(window), EventSource(event_source))
}

/// `RawWindowHandle` implementor returned by the `setup` function.
pub struct EditorWindow(EditorWindowImpl);

/// The `EditorWindow` can be passed to any rendering backend that accepts raw window handles
/// through the `raw-window-handle` crate.
unsafe impl HasRawWindowHandle for EditorWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0.raw_window_handle()
    }
}

/// Source of events from a corresponding window, created by the `setup` function.
pub struct EventSource(EventSourceImpl);

impl EventSource {
    /// Returns the next `WindowEvent`, if one is available. This should be called in a `while let`
    /// loop until empty.
    pub fn poll_event(&self) -> Option<WindowEvent> {
        self.0.poll_event()
    }
}
