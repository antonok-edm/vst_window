//! Provides window setup logic specific to the Windows platform.

use std::{
    convert::TryInto,
    sync::{Arc, Mutex, Weak},
};

use raw_window_handle::{HasRawWindowHandle, RawWindowHandle, Win32Handle};
use winapi::{
    shared::{minwindef, ntdef, windef, winerror},
    um::{libloaderapi, winuser},
};

use crate::{
    platform::os::{format_last_error, get_last_error, wrap_last_error},
    InvalidParentError, SetupError,
};

pub(in crate::platform) struct ChildWindow {
    pub hwnd: windef::HWND,
    _class: Arc<VstWindowClass>,
}

impl Drop for ChildWindow {
    fn drop(&mut self) {
        let error = unsafe { winuser::DestroyWindow(self.hwnd) };
        if error == minwindef::FALSE && log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "Error: {}",
                crate::ErrorChainPrinter(SetupError::with_context_boxed(
                    format_last_error("DestroyWindow").into(),
                    "failed to destroy child window"
                ))
            );
        }
    }
}

unsafe impl HasRawWindowHandle for ChildWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32Handle::empty();
        handle.hwnd = self.hwnd as *mut std::ffi::c_void;
        handle.hinstance =
            unsafe { libloaderapi::GetModuleHandleW(std::ptr::null()) } as *mut std::ffi::c_void;
        RawWindowHandle::Win32(handle)
    }
}

impl ChildWindow {
    /// On Windows, child window creation is as simple as calling `CreateWindowEx` with the parent
    /// HWND and the right set of flags.
    ///
    /// However, it's necessary to register a "window class" before the window can be created - see
    /// `WINDOW_CLASS`.
    ///
    /// # Safety
    /// `parent` must be a valid HWND
    pub unsafe fn build(
        parent: *mut std::os::raw::c_void,
        size_xy: (i32, i32),
    ) -> Result<Self, SetupError> {
        // TODO validate window size potentially ERROR_INCORRECT_SIZE

        let instance = unsafe { libloaderapi::GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            return Err(wrap_last_error("GetModuleHandleW"));
        }

        let class = get_window_class(instance)?;

        let hwnd = unsafe {
            let window_type = winuser::WS_VISIBLE | winuser::WS_CHILD;

            if parent.is_null() {
                return Err(SetupError::new(InvalidParentError::new(parent)));
            }

            let parent = parent as windef::HWND;

            winuser::CreateWindowExW(
                0,
                MAKEINTATOMW(class.0),
                wchar::wchz!("").as_ptr(),
                window_type,
                0,
                0,
                size_xy.0,
                size_xy.1,
                parent,
                std::ptr::null_mut(),
                instance,
                std::ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            let (errno, _) = get_last_error();
            // special case invalid parent window case for easier debugging
            if errno == winerror::ERROR_INVALID_WINDOW_HANDLE {
                return Err(SetupError::new(InvalidParentError::new(parent)));
            } else {
                return Err(wrap_last_error("CreateWindowExW"));
            }
        }

        unsafe {
            winuser::EnableWindow(hwnd, minwindef::TRUE);
        }

        Ok(Self {
            hwnd,
            _class: class,
        })
    }
}

#[allow(non_snake_case)]
fn MAKEINTATOMW(atom: minwindef::ATOM) -> ntdef::LPCWSTR {
    // MAKEINTATOMW is missing: https://github.com/retep998/winapi-rs/issues/576
    atom as minwindef::WORD as winapi::shared::basetsd::ULONG_PTR as ntdef::LPCWSTR
}

/// Lazily registered window class used for the VST plugin window.
/// Will get unregistered when no more windows are in use.
///
/// Crucially, the class must define a "window process", or main event loop. We use the `wnd_proc`
/// function from the `event_source` module for this purpose.
fn get_window_class(instance: minwindef::HINSTANCE) -> Result<Arc<VstWindowClass>, SetupError> {
    lazy_static::lazy_static! {
        static ref WINDOW_CLASS: Mutex<Weak<VstWindowClass>> = Mutex::new(Weak::new());
    }

    let mut guard = WINDOW_CLASS
        .lock()
        .expect("other thread panicked in get_window_class");

    if let Some(class) = guard.upgrade() {
        Ok(class)
    } else {
        let class = Arc::new(VstWindowClass::create(instance)?);
        *guard = Arc::downgrade(&class);
        Ok(class)
    }
}

struct VstWindowClass(minwindef::ATOM);

impl VstWindowClass {
    fn create(instance: minwindef::HINSTANCE) -> Result<Self, SetupError> {
        let atom = unsafe {
            let class_ex = winuser::WNDCLASSEXW {
                cbSize: std::mem::size_of::<winuser::WNDCLASSEXW>()
                    .try_into()
                    .unwrap(),
                style: winuser::CS_OWNDC,
                lpfnWndProc: Some(super::event_source::wnd_proc),
                lpszClassName: wchar::wchz!("vst_window_class").as_ptr(),
                hInstance: instance,
                ..std::mem::zeroed()
            };

            winuser::RegisterClassExW(&class_ex as *const winuser::WNDCLASSEXW)
        };

        if atom == 0 {
            return Err(wrap_last_error("RegisterClassExW"));
        }

        Ok(Self(atom))
    }
}

impl Drop for VstWindowClass {
    fn drop(&mut self) {
        let error =
            unsafe { winuser::UnregisterClassW(MAKEINTATOMW(self.0), std::ptr::null_mut()) };
        if error == minwindef::FALSE && log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "Error: {}",
                crate::ErrorChainPrinter(SetupError::with_context_boxed(
                    format_last_error("UnregisterClassW").into(),
                    "failed to unregister window class"
                ))
            );
        }
    }
}
