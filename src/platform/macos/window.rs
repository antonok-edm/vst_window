//! Provides window setup logic specific to the MacOS platform.

use cocoa::base::id;
use objc::{msg_send, sel, sel_impl};
use raw_window_handle::{AppKitHandle, HasRawWindowHandle, RawWindowHandle};

use crate::platform::EditorWindowBackend;

pub(in crate::platform) struct EditorWindowImpl {
    ns_window: id,
    pub ns_view: id,
}

impl Drop for EditorWindowImpl {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.ns_window, release];
            let _: () = msg_send![self.ns_view, release];
        }
    }
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
    unsafe fn build(parent: *mut std::os::raw::c_void, _size_xy: (i32, i32)) -> anyhow::Result<Self> {
        // TODO validate window size
        //return error if parent is nil to aid debugging
        if parent.is_null() {
            return Err(crate::Error::Other {
                source: anyhow::anyhow!("invalid parent (null pointer)"),
                backend: crate::Backend::Cocoa,
            });
        }
        let (ns_window, ns_view) = unsafe {
            let ns_view = parent as id;
            let window: id = msg_send![ns_view, window];

            let _: id = msg_send![ns_view, retain];
            let _: id = msg_send![window, retain];

            (window, ns_view)
        };

        Ok(Self { ns_window, ns_view })
    }
}
