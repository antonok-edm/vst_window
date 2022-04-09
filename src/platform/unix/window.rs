//! Provides window setup logic specific to the Unix platform.

use std::{convert::TryInto, sync::Arc};

use raw_window_handle::{unix::XcbHandle, HasRawWindowHandle, RawWindowHandle};
use x11rb::{
    connection::Connection, protocol::xproto::ConnectionExt as _, rust_connection::ReplyError,
    wrapper::ConnectionExt as _,
};

use crate::{InvalidParentError, InvalidSizeError, SetupError};

pub(in crate::platform) struct ChildWindow {
    pub connection: Arc<x11rb::xcb_ffi::XCBConnection>,
    window_id: x11rb::protocol::xproto::Window,
}

impl Drop for ChildWindow {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window_id);
    }
}

unsafe impl HasRawWindowHandle for ChildWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Xcb(XcbHandle {
            connection: self.connection.get_raw_xcb_connection() as *mut std::ffi::c_void,
            window: self.window_id,
            ..XcbHandle::empty()
        })
    }
}

x11rb::atom_manager! {
    AtomCollection: AtomCollectionCookie {
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DIALOG,
    }
}

impl ChildWindow {
    /// The VST API provides an XCB handle on Unix, so the window is setup using `xcb`.
    ///
    /// All XCB operations rely on a connection handle to the XCB backend. Conveniently, the XCB
    /// `create_window` function takes a parent window id argument.
    ///
    /// XCB operations can be called from any thread - unlike the other platforms, there are
    /// practically no restrictions on the control flow of the windowing logic.
    pub fn build(
        parent: *mut std::os::raw::c_void,
        size_xy: (i32, i32),
    ) -> Result<Self, SetupError> {
        let size_xy: (u16, u16) = {
            (
                size_xy
                    .0
                    .try_into()
                    .map_err(|_| SetupError::new(InvalidSizeError(size_xy)))?,
                size_xy
                    .1
                    .try_into()
                    .map_err(|_| SetupError::new(InvalidSizeError(size_xy)))?,
            )
        };

        use x11rb::protocol::xproto;

        let (connection, _screen_num) =
            x11rb::xcb_ffi::XCBConnection::connect(None).map_err(|conn_err| {
                SetupError::with_context(conn_err, "couldn't connect to display server")
            })?;

        let parent_id: u32 = (parent as usize)
            .try_into()
            .map_err(|_| SetupError::new(InvalidParentError::new(parent)))?;
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
        let atom_collection =
            AtomCollection::new(&connection)?
                .reply()
                .map_err(|err| match err {
                    ReplyError::ConnectionError(conn_err) => conn_err.into(),
                    _ => SetupError::with_context(err, "failed to intern strings"),
                })?;

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

        connection.sync()?;

        while let Some(event) = connection.poll_for_event()? {
            if let x11rb::protocol::Event::Error(err) = event {
                // Special case for error type "Window" on create_window call which can only mean the provided parent window id is invalid to aid debugging
                if err.sequence == (create_window_seq & u16::MAX as u64) as u16
                    && err.error_kind == x11rb::protocol::ErrorKind::Window
                {
                    return Err(SetupError::new(InvalidParentError::new(parent)));
                }
                // Pretty print if extension and request name are known, otherwise print raw codes
                if let (Some(ext_name), Some(req_name)) = (&err.extension_name, err.request_name) {
                    return Err(SetupError::new_boxed(
                        format!(
                            "request {} ({}) with value {} failed: {:?}",
                            req_name, ext_name, err.bad_value, err.error_kind
                        )
                        .into(),
                    ));
                } else {
                    return Err(SetupError::new_boxed(
                        format!(
                            "request opcode (major {}, minor {}) with value {} failed: {:?}",
                            err.major_opcode, err.minor_opcode, err.bad_value, err.error_kind
                        )
                        .into(),
                    ));
                }
            }
        }

        Ok(Self {
            connection: Arc::new(connection),
            window_id,
        })
    }
}
