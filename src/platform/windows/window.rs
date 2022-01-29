//! Provides window setup logic specific to the Windows platform.

use std::os::windows::ffi::OsStrExt;

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, Win32Handle};
use winapi::{
    shared::{minwindef, windef},
    um::{libloaderapi, winuser},
};

use crate::platform::EditorWindowBackend;

pub(in crate::platform) struct EditorWindowImpl {
    pub hwnd: windef::HWND,
}

unsafe impl HasRawWindowHandle for EditorWindowImpl {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32Handle::empty();
        handle.hwnd = self.hwnd as *mut std::ffi::c_void;
        handle.hinstance =
            unsafe { libloaderapi::GetModuleHandleW(std::ptr::null()) } as *mut std::ffi::c_void;
        RawWindowHandle::Windows(handle)
    }
}

impl EditorWindowBackend for EditorWindowImpl {
    /// On Windows, child window creation is as simple as calling `CreateWindowEx` with the parent
    /// HWND and the right set of flags.
    ///
    /// However, it's necessary to register a "window class" before the window can be created - see
    /// `WINDOW_CLASS`.
    fn build(parent: *mut std::os::raw::c_void, _size_xy: (i32, i32)) -> Self {
        let parent = parent as windef::HWND;

        let window_type = winuser::WS_VISIBLE | winuser::WS_CHILD;

        let emptystr: Vec<u16> = std::ffi::OsStr::new("")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let hwnd = unsafe {
            winuser::CreateWindowExW(
                winuser::WS_EX_TOOLWINDOW,
                // MAKEINTATOMW is missing: https://github.com/retep998/winapi-rs/issues/576
                (*WINDOW_CLASS) as minwindef::WORD as winapi::shared::basetsd::ULONG_PTR
                    as winapi::shared::ntdef::LPCWSTR,
                emptystr.as_ptr(),
                window_type,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                winuser::CW_USEDEFAULT,
                parent,
                std::ptr::null_mut(),
                libloaderapi::GetModuleHandleW(std::ptr::null()),
                std::ptr::null_mut(),
            )
        };

        unsafe { winuser::ShowWindow(hwnd, winuser::SW_MAXIMIZE) };
        unsafe { winuser::EnableWindow(hwnd, minwindef::TRUE) };

        Self { hwnd }
    }
}

/// Lazily registered window class used for the VST plugin window.
///
/// Crucially, the class must define a "window process", or main event loop. We use the `wnd_proc`
/// function from the `event_source` module for this purpose.
static WINDOW_CLASS: once_cell::sync::Lazy<minwindef::ATOM> = once_cell::sync::Lazy::new(|| {
    let class_name: Vec<u16> = std::ffi::OsStr::new("vst_window_class")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let wcex = winuser::WNDCLASSEXW {
        cbSize: std::mem::size_of::<winuser::WNDCLASSEXW>() as u32,
        style: winuser::CS_OWNDC,
        lpfnWndProc: Some(super::event_source::wnd_proc),
        lpszClassName: class_name.as_ptr(),
        hInstance: unsafe { libloaderapi::GetModuleHandleW(std::ptr::null()) },
        ..unsafe { std::mem::zeroed() }
    };

    let atom = unsafe { winuser::RegisterClassExW(&wcex as *const winuser::WNDCLASSEXW) };
    assert!(atom != 0);

    atom
});
