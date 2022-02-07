//! Provides window setup logic specific to the MacOS platform.

use cocoa::base::id;
use objc::{msg_send, sel, sel_impl};
use raw_window_handle::{AppKitHandle, HasRawWindowHandle, RawWindowHandle};

use crate::platform::EditorWindowBackend;

pub(in crate::platform) struct EditorWindowImpl {
    ns_window: id,
    pub ns_view: id,
}

unsafe impl HasRawWindowHandle for EditorWindowImpl {
    fn raw_window_handle(&self) -> RawWindowHandle {
        use std::os::raw::c_void;

        let mut handle = AppKitHandle::empty();
        handle.ns_window = self.ns_window as *mut c_void;
        handle.ns_view = self.ns_view as *mut c_void;
        RawWindowHandle::AppKit(handle)
    }
}

impl EditorWindowBackend for EditorWindowImpl {
    /// Technically, this doesn't even use `parent` as a parent window - the host DAW creates an
    /// NSWindow with an embedded NSView, and passes along the id of the NSView. We just directly
    /// pass along that same NSView for rendering!
    fn build(parent: *mut std::os::raw::c_void, _size_xy: (i32, i32)) -> Self {
        let (ns_window, ns_view) = unsafe {
            let ns_view = parent as id;
            let window: id = msg_send![ns_view, window];

            (window, ns_view)
        };

        Self { ns_window, ns_view }
    }
}
