//! Provides window setup logic specific to the Unix platform.

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, XcbHandle};

use crate::platform::EditorWindowBackend;

/// "User-specified size" flag for WM_NORMAL_HINTS
const USSIZE: u32 = 2;
/// "Program-specified" min size flag for WM_NORMAL_HINTS
const PMINSIZE: u32 = 16;
/// "Program-specified" max size flag for WM_NORMAL_HINTS
const PMAXSIZE: u32 = 32;

pub(in crate::platform) struct EditorWindowImpl {
    /// This is always `Some` throughout the usable lifetime of the `EditorWindow`; it will only be
    /// `None` during the `Drop` implementation.
    pub connection: Option<xcb::base::Connection>,
    window_id: u32,
}

unsafe impl HasRawWindowHandle for EditorWindowImpl {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = XcbHandle::empty();
        handle.connection =
            self.connection.as_ref().unwrap().get_raw_conn() as *mut std::ffi::c_void;
        handle.window = self.window_id;
        RawWindowHandle::Xcb(handle)
    }
}

impl Drop for EditorWindowImpl {
    /// The `xcb` crate will disconnect the connection to the backend when the `Connection` type is
    /// dropped. Here, we convert the wrapper back into a raw connection, to prevent it from being
    /// disconnected a second time in the `EventSource`.
    fn drop(&mut self) {
        self.connection.take().unwrap().into_raw_conn();
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
    fn build(parent: *mut std::os::raw::c_void, size_xy: (i32, i32)) -> Self {
        let (connection, screen_num) = xcb::base::Connection::connect(None).unwrap();
        let setup = connection.get_setup();
        let screen = setup.roots().nth(screen_num as usize).expect("Get screen");

        let foreground = connection.generate_id();
        let values = [
            (xcb::GC_FOREGROUND, screen.black_pixel()),
            (xcb::GC_GRAPHICS_EXPOSURES, 0),
        ];
        xcb::create_gc(&connection, foreground, screen.root(), &values[..]);

        let event_mask = xcb::EVENT_MASK_EXPOSURE
            | xcb::EVENT_MASK_KEY_PRESS
            | xcb::EVENT_MASK_BUTTON_PRESS
            | xcb::EVENT_MASK_BUTTON_RELEASE
            | xcb::EVENT_MASK_POINTER_MOTION;
        let wid = connection.generate_id();
        let parent = parent as u32;
        let values = [
            (xcb::CW_BACK_PIXEL, screen.black_pixel()),
            (xcb::CW_EVENT_MASK, event_mask),
        ];

        let _cookie = xcb::xproto::create_window(
            &connection,
            xcb::COPY_FROM_PARENT as u8,
            wid,
            parent,
            0,
            0,
            size_xy.0 as u16,
            size_xy.1 as u16,
            0,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &values[..],
        );

        let net_wm_window_type = xcb_intern_string(&connection, "_NET_WM_WINDOW_TYPE");
        let net_wm_window_type_dialog =
            xcb_intern_string(&connection, "_NET_WM_WINDOW_TYPE_DIALOG");
        xcb::change_property(
            &connection,
            xcb::PROP_MODE_REPLACE as u8,
            wid,
            net_wm_window_type,
            xcb::ATOM_ATOM,
            32,
            &[net_wm_window_type_dialog],
        );

        let wm_normal_hints = xcb_intern_string(&connection, "WM_NORMAL_HINTS");
        let size_hints = {
            let size_x = size_xy.0 as u32;
            let size_y = size_xy.1 as u32;
            let flags = USSIZE | PMINSIZE | PMAXSIZE;
            [
                flags, 0, 0, 0, 0, size_x, size_y, size_x, size_y, 0, 0, 0, 0, size_x, size_y,
            ]
        };
        xcb::change_property(
            &connection,
            xcb::PROP_MODE_REPLACE as u8,
            wid,
            wm_normal_hints,
            xcb::xproto::ATOM_WM_SIZE_HINTS,
            32,
            &size_hints,
        );

        xcb::xproto::map_window(&connection, wid);
        connection.flush();

        drop(_cookie);

        Self {
            connection: Some(connection),
            window_id: wid,
        }
    }
}

/// With XCB, some window properties are identified using `Atom`s, which are identifiers for
/// strings that have been previously interned.
fn xcb_intern_string(connection: &xcb::Connection, value: &str) -> xcb::Atom {
    match xcb::intern_atom(&connection, false, value).get_reply() {
        Ok(reply) => reply.atom(),
        Err(_) => panic!("could not intern {} atom", value),
    }
}
