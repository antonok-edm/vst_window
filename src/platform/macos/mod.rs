//! Platform-specific implementation for MacOS.

use std::os::raw::c_void;
use std::sync::mpsc::{channel, Receiver};

use cocoa::base::id;
use objc::{
    msg_send,
    rc::StrongPtr,
    sel, sel_impl,
};
use raw_window_handle::{AppKitHandle, HasRawWindowHandle, RawWindowHandle};

use crate::event::WindowEvent;
use crate::platform::os::event_proxy_class::instantiate_event_proxy;

mod event_proxy_class;

pub(in crate::platform) struct EditorWindowImpl {
    event_proxy: StrongPtr,
    incoming_events: Receiver<WindowEvent>,

    ns_window: StrongPtr,
}

unsafe impl HasRawWindowHandle for EditorWindowImpl {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = AppKitHandle::empty();
        handle.ns_window = *self.ns_window as *mut c_void;
        handle.ns_view = *self.event_proxy as *mut c_void;
        RawWindowHandle::AppKit(handle)
    }
}

impl crate::platform::EditorWindowBackend for EditorWindowImpl {
    unsafe fn build(
        parent: *mut std::os::raw::c_void,
        size_xy: (i32, i32),
    ) -> anyhow::Result<Self> {
        // TODO validate window size

        // return error if parent is nil to aid debugging
        if parent.is_null() {
            return Err(crate::Error::Other {
                source: anyhow::anyhow!("invalid parent (null pointer)"),
                backend: crate::Backend::Cocoa,
            }
            .into());
        }

        let parent = parent as id;

        let ns_window = unsafe {
            let window: id = msg_send![parent, window];

            StrongPtr::retain(window)
        };
        
        let (event_sender, incoming_events) = channel();

        let event_proxy: StrongPtr = unsafe { instantiate_event_proxy(parent, event_sender, size_xy)? };

        Ok(Self {
            event_proxy,
            incoming_events,

            ns_window,
        })
    }

    fn poll_event(&self) -> Option<WindowEvent> {
        match self.incoming_events.try_recv() {
            Ok(ev) => Some(ev),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => unreachable!(
                "self.event_subview is released when self is dropped, panic should abort"
            ),
        }
    }
}
