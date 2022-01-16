//! Provides window setup logic specific to the Windows platform.

use std::{
    convert::TryInto,
    sync::{Arc, Mutex, Weak},
};

use raw_window_handle::{windows::WindowsHandle, HasRawWindowHandle, RawWindowHandle};
use winapi::{
    shared::{minwindef, ntdef, windef, winerror},
    um::{libloaderapi, winuser},
};

use crate::platform::{os::get_last_error, EditorWindowBackend};

pub(in crate::platform) struct EditorWindowImpl {
    pub hwnd: windef::HWND,
    _class: Arc<VstWindowClass>,
}

impl Drop for EditorWindowImpl {
    fn drop(&mut self) {
        let error = unsafe { winuser::DestroyWindow(self.hwnd) };
        if error == minwindef::FALSE && log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "Failed to destroy window: {}",
                crate::Error::Other {
                    source: anyhow::anyhow!(get_last_error().1).context("DestroyWindow"),
                    backend: crate::Backend::WinApi,
                }
            );
        }
    }
}

unsafe impl HasRawWindowHandle for EditorWindowImpl {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Windows(WindowsHandle {
            hwnd: self.hwnd as *mut std::ffi::c_void,
            hinstance: unsafe { libloaderapi::GetModuleHandleW(std::ptr::null()) }
                as *mut std::ffi::c_void,
            ..WindowsHandle::empty()
        })
    }
}

impl EditorWindowBackend for EditorWindowImpl {
    /// On Windows, child window creation is as simple as calling `CreateWindowEx` with the parent
    /// HWND and the right set of flags.
    ///
    /// However, it's necessary to register a "window class" before the window can be created - see
    /// `WINDOW_CLASS`.
    unsafe fn build(
        parent: *mut std::os::raw::c_void,
        size_xy: (i32, i32),
    ) -> anyhow::Result<Self> {
        // TODO validate window size potentially ERROR_INCORRECT_SIZE

        let instance = unsafe { libloaderapi::GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            return Err(crate::Error::Other {
                source: anyhow::anyhow!(get_last_error().1).context("GetModuleHandleW"),
                backend: crate::Backend::WinApi,
            }
            .into());
        }

        let class = get_window_class(instance)?;

        let hwnd = unsafe {
            let window_type = winuser::WS_VISIBLE | winuser::WS_CHILD;
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
                return Err(crate::Error::Other {
                    source: anyhow::anyhow!("invalid parent hwnd supplied"),
                    backend: crate::Backend::WinApi,
                }
                .into());
            } else {
                return Err(crate::Error::Other {
                    source: anyhow::anyhow!(get_last_error().1).context("CreateWindowExW"),
                    backend: crate::Backend::WinApi,
                }
                .into());
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
fn get_window_class(instance: minwindef::HINSTANCE) -> anyhow::Result<Arc<VstWindowClass>> {
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
    fn create(instance: minwindef::HINSTANCE) -> anyhow::Result<Self> {
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
            return Err(crate::Error::Other {
                source: anyhow::anyhow!(get_last_error().1).context("RegisterClassExW"),
                backend: crate::Backend::WinApi,
            }
            .into());
        }

        Ok(Self(atom))
    }
}

impl Drop for VstWindowClass {
    fn drop(&mut self) {
        let error = unsafe { winuser::UnregisterClassW(MAKEINTATOMW(self.0), std::ptr::null_mut()) };
        if error == minwindef::FALSE && log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "Failed to unregister window class: {}",
                crate::Error::Other {
                    source: anyhow::anyhow!(get_last_error().1).context("UnregisterClassW"),
                    backend: crate::Backend::WinApi,
                }
            );
        }
    }
}
