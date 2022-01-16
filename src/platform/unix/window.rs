//! Provides window setup logic specific to the Unix platform.

use std::{convert::TryInto, sync::Arc};

use anyhow::Context;
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, XcbHandle};
use x11rb::{
    connection::Connection, protocol::xproto::ConnectionExt as _, wrapper::ConnectionExt as _,
};

use crate::platform::EditorWindowBackend;

pub(in crate::platform) struct EditorWindowImpl {
    pub connection: Arc<x11rb::xcb_ffi::XCBConnection>,
    window_id: x11rb::protocol::xproto::Window,
}

impl Drop for EditorWindowImpl {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window_id);
    }
}

unsafe impl HasRawWindowHandle for EditorWindowImpl {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = XcbHandle::empty();
        handle.connection = self.connection.get_raw_xcb_connection() as *mut std::ffi::c_void;
        handle.window = self.window_id;
        RawWindowHandle::Xcb(handle)
    }
}

x11rb::atom_manager! {
    AtomCollection: AtomCollectionCookie {
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DIALOG,
    }
}

impl EditorWindowBackend for EditorWindowImpl {
    /// The VST API provides an XCB handle on Unix, so the window is setup using `xcb`.
    ///
    /// All XCB operations rely on a connection handle to the XCB backend. Conveniently, the XCB
    /// `create_window` function takes a parent window id argument.
    ///
    /// XCB operations can be called from any thread - unlike the other platforms, there are
    /// practically no restrictions on the control flow of the windowing logic.
    unsafe fn build(
        parent: *mut std::os::raw::c_void,
        size_xy: (i32, i32),
    ) -> anyhow::Result<Self> {
        let size_xy: (u16, u16) = {
            (
                size_xy
                    .0
                    .try_into()
                    .map_err(|_| crate::Error::InvalidWindowSize {
                        requested_size_xy: size_xy,
                        limits: 0..i16::MAX.into(),
                    })?,
                size_xy
                    .1
                    .try_into()
                    .map_err(|_| crate::Error::InvalidWindowSize {
                        requested_size_xy: size_xy,
                        limits: 0..i16::MAX.into(),
                    })?,
            )
        };

        use x11rb::protocol::xproto;

        let (connection, _screen_num) = x11rb::xcb_ffi::XCBConnection::connect(None)
            .context("couldn't establish connection to display server")?;

        let parent_id: u32 = (parent as usize)
            .try_into()
            .map_err(|_| crate::Error::Other {
                source: anyhow::anyhow!("invalid parent id supplied"),
                backend: crate::Backend::X11,
            })?;
        use x11rb::protocol::xproto::EventMask;
        // listen to appropriate events
        let event_mask = EventMask::EXPOSURE
            | EventMask::KEY_PRESS
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::POINTER_MOTION;
        let aux = xproto::CreateWindowAux {
            //background_pixel: screen.black_pixel
            event_mask: Some(event_mask.into()),
            ..Default::default()
        };
        let window_id = connection.generate_id()?;
        let create_window_seq = connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window_id,
                parent_id,
                0,
                0,
                size_xy.0,
                size_xy.1,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                x11rb::COPY_FROM_PARENT,
                &aux,
            )?
            .sequence_number();

        // (property name) strings must first be interned to be used
        let atom_collection = AtomCollection::new(&connection)
            .context("failed to intern strings")?
            .reply()
            .context("failed to intern strings")?;

        // indicate that this is a dialog type window
        // see https://specifications.freedesktop.org/wm-spec/1.3/ar01s05.html#idm44949527944400
        connection.change_property32(
            xproto::PropMode::REPLACE,
            window_id,
            atom_collection._NET_WM_WINDOW_TYPE,
            xproto::AtomEnum::ATOM,
            &[atom_collection._NET_WM_WINDOW_TYPE_DIALOG],
        )?;

        // prevent the window from being resized
        let size_hints = x11rb::properties::WmSizeHints {
            min_size: Some((size_xy.0.into(), size_xy.1.into())),
            max_size: Some((size_xy.0.into(), size_xy.1.into())),
            ..Default::default()
        };
        size_hints.set_normal_hints(&connection, window_id)?;

        // show the window
        connection.map_window(window_id)?;

        connection.sync().context("failed to sync connection")?;

        while let Some(event) = connection.poll_for_event()? {
            if let x11rb::protocol::Event::Error(err) = event {
                // Special case for error type "Window" on create_window call which can only mean the provided parent window id is invalid to aid debugging
                if err.sequence == (create_window_seq & u16::MAX as u64) as u16
                    && err.error_kind == x11rb::protocol::ErrorKind::Window
                {
                    return Err(anyhow::anyhow!(crate::Error::Other {
                        source: anyhow::anyhow!("invalid parent id supplied"),
                        backend: crate::Backend::X11,
                    }));
                }
                // Pretty print if extension and request name are known, otherwise print raw codes
                if let (Some(ext_name), Some(req_name)) = (&err.extension_name, err.request_name) {
                    return Err(anyhow::anyhow!(
                        "request {} ({}) failed: {:?}",
                        req_name,
                        ext_name,
                        err.error_kind
                    ));
                } else {
                    return Err(anyhow::anyhow!("{:?}", err));
                }
            }
        }

        Ok(Self {
            connection: Arc::new(connection),
            window_id,
        })
    }
}
