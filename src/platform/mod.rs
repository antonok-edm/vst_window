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

use anyhow::Context;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

use crate::event::WindowEvent;

#[cfg_attr(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    path = "unix/mod.rs"
)]
#[cfg_attr(target_os = "macos", path = "macos/mod.rs")]
#[cfg_attr(target_os = "windows", path = "windows/mod.rs")]
mod os;

use os::event_source::EventSourceImpl;
use os::window::EditorWindowImpl;

/// Crate-internal cross-platform window handle creation API required on each platform.
trait EditorWindowBackend: raw_window_handle::HasRawWindowHandle + Sized {
    /// Builds a platform-specific window, using a provided window handle as a parent window.
    ///
    /// # Safety
    /// `parent` must be a valid window identifier
    unsafe fn build(parent: *mut std::os::raw::c_void, size_xy: (i32, i32))
        -> anyhow::Result<Self>;
}

/// Crate-internal cross-platform event source API required on each platform.
trait EventSourceBackend: Sized {
    /// Builds a platform-specific event source corresponding to the provided window.
    fn new(window: &EditorWindowImpl, size_xy: (i32, i32)) -> anyhow::Result<Self>;
    /// Returns the next `WindowEvent`, if one is available.
    fn poll_event(&self) -> Option<WindowEvent>;
}

/// Build a platform-specific window and return a cross-platform `RawWindowHandle` implementor,
/// used as a surface for rendering, as well as a cross-platform `EventSource`, which is used to
/// poll `WindowEvent`s.
///
/// `parent` should be a window handle as passed from a host to a plugin by the `vst` crate.
/// `size_xy` should be the size returned by the size function on the VST editor. Assumes position of VST editor to be (0, 0).
///
/// # Safety
/// `parent` must be a valid window identifier on the corresponding platform i.e.
/// - macOS (Cocoa): A valid pointer to an NSView object
/// - Windows (win32): A valid window handle (HWND)
/// - unix (X11): A valid "WINDOW" value
/// Passing invalid values results in undefined behaviour.
pub unsafe fn setup(
    parent: *mut std::os::raw::c_void,
    size_xy: (i32, i32),
) -> crate::Result<EditorWindow> {
    let window = EditorWindowImpl::build(parent, size_xy).context("couldn't initialize window")?;
    let event_source =
        EventSourceImpl::new(&window, size_xy).context("couldn't initialize event source")?;
    Ok(EditorWindow(window, event_source))
}

/// `RawWindowHandle` implementor returned by the `setup` function.
/// Source of events from a corresponding window, created by the `setup` function.
/// The window will be destroyed once this is dropped.
pub struct EditorWindow(EditorWindowImpl, EventSourceImpl);

impl EditorWindow {
    /// Returns the next `WindowEvent`, if one is available. This should be called in a `while let`
    /// loop until empty.
    pub fn poll_event(&self) -> Option<WindowEvent> {
        self.1.poll_event()
    }
}

/// The `EditorWindow` can be passed to any rendering backend that accepts raw window handles
/// through the `raw-window-handle` crate.
unsafe impl HasRawWindowHandle for EditorWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0.raw_window_handle()
    }
}
