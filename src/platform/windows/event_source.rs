//! Provides a source for window events on Windows platforms.

use std::sync::mpsc::{channel, Receiver, Sender};

use winapi::{
    shared::{minwindef, windef},
    um::{errhandlingapi, winuser},
};

use crate::{
    event::{MouseButton, WindowEvent},
    platform::os::format_last_error,
    SetupError,
};

use super::{window::ChildWindow, wrap_last_error};

pub(in crate::platform) struct EventSource {
    hwnd: windef::HWND,
    incoming_window_events: Receiver<WindowEvent>,
}

impl EventSource {
    /// Window events must be received in the "window process" defined by the window's class, in
    /// its own main thread. However, the inversion of control flow in the VST API means that we
    /// can't run the windowing logic in the window process. Instead, we just use it to forward the
    /// events over a channel so that they can be polled lazily from the editor's `idle` function.
    /// The channel sender is heap-allocated, and its pointer is stored as extra "user data"
    /// associated with the HWND.
    pub fn new(window: &ChildWindow, size_xy: (i32, i32)) -> Result<Self, SetupError> {
        let (event_sender, incoming_window_events) = channel();
        let event_sender_ptr = Box::into_raw(Box::new((event_sender, size_xy)));
        unsafe {
            errhandlingapi::SetLastError(0);
            let previous_value = winuser::SetWindowLongPtrW(
                window.hwnd,
                winuser::GWLP_USERDATA,
                event_sender_ptr as winapi::shared::basetsd::LONG_PTR,
            );

            if previous_value == 0 && errhandlingapi::GetLastError() != 0 {
                return Err(wrap_last_error("SetWindowLongPtrW"));
            }
        }

        Ok(Self {
            hwnd: window.hwnd,
            incoming_window_events,
        })
    }

    pub fn poll_event(&self) -> Option<WindowEvent> {
        match self.incoming_window_events.try_recv() {
            Ok(ev) => Some(ev),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                unreachable!("sender will get dropped with self")
            }
        }
    }
}

impl Drop for EventSource {
    fn drop(&mut self) {
        unsafe {
            // set to null to prevent dangling pointer
            errhandlingapi::SetLastError(0);
            let event_sender_ptr = winuser::SetWindowLongPtrW(
                self.hwnd,
                winuser::GWLP_USERDATA,
                std::ptr::null_mut::<winapi::ctypes::c_void>() as winapi::shared::basetsd::LONG_PTR,
            ) as *mut (Sender<WindowEvent>, (i32, i32));

            if !event_sender_ptr.is_null() {
                drop(Box::from_raw(event_sender_ptr));
            } else if log::log_enabled!(log::Level::Debug) && errhandlingapi::GetLastError() != 0 {
                log::debug!(
                    "Error: {}",
                    crate::ErrorChainPrinter(SetupError::with_context_boxed(
                        format_last_error("SetWindowLongPtrW").into(),
                        "failed to cleanup event sender"
                    ))
                );
            }
        }
    }
}

/// "Window process", or main loop, for the VST window. Whenever a window event occurs, this
/// function will be called once. This implementation simply gets the `Sender<WindowEvent>`
/// associated with the window handle, and forwards events over that channel.
///
/// After most events, it's important to forward the arguments to `DefWindowProc`, or the default
/// window process.
pub(super) unsafe extern "system" fn wnd_proc(
    hwnd: windef::HWND,
    umsg: minwindef::UINT,
    wparam: minwindef::WPARAM,
    lparam: minwindef::LPARAM,
) -> minwindef::LRESULT {
    let (event_sender, size_xy) = unsafe {
        // TODO what if somebody else modifies GWLP_USERDATA?
        let event_sender_ptr = winuser::GetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA)
            as *mut (Sender<WindowEvent>, (i32, i32));
        if event_sender_ptr.is_null() {
            log::debug!(
                "Ignored window event ({}) because event sender is not yet initialized (Win32)",
                umsg
            );
            return winuser::DefWindowProcW(hwnd, umsg, wparam, lparam);
        }

        &mut *(event_sender_ptr)
    };

    match umsg {
        // https://docs.microsoft.com/en-us/windows/win32/dlgbox/wm-getdlgcode
        // TODO check whether this is needed
        //winuser::WM_GETDLGCODE => return winuser::DLGC_WANTALLKEYS,
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/wm-mousemove
        winuser::WM_MOUSEMOVE => {
            let x_pos = winapi::shared::windowsx::GET_X_LPARAM(lparam);
            let y_pos = winapi::shared::windowsx::GET_Y_LPARAM(lparam);
            let x = (x_pos as f32) / (size_xy.0 as f32);
            let y = (y_pos as f32) / (size_xy.1 as f32);
            event_sender
                .send(WindowEvent::CursorMovement(x, y))
                .unwrap();
        }
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttondown
        winuser::WM_LBUTTONDOWN => {
            event_sender
                .send(WindowEvent::MouseClick(MouseButton::Left))
                .unwrap();
            unsafe { winuser::SetCapture(hwnd) };
        }
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/wm-lbuttonup
        winuser::WM_LBUTTONUP => {
            event_sender
                .send(WindowEvent::MouseRelease(MouseButton::Left))
                .unwrap();
            unsafe { winuser::ReleaseCapture() };
        }
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/wm-rbuttondown
        winuser::WM_RBUTTONDOWN => {
            event_sender
                .send(WindowEvent::MouseClick(MouseButton::Right))
                .unwrap();
            unsafe { winuser::SetCapture(hwnd) };
        }
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/wm-rbuttonup
        winuser::WM_RBUTTONUP => {
            event_sender
                .send(WindowEvent::MouseRelease(MouseButton::Right))
                .unwrap();
            unsafe { winuser::ReleaseCapture() };
        }
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/wm-mbuttondown
        winuser::WM_MBUTTONDOWN => {
            event_sender
                .send(WindowEvent::MouseClick(MouseButton::Middle))
                .unwrap();
            unsafe { winuser::SetCapture(hwnd) };
        }
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/wm-mbuttonup
        winuser::WM_MBUTTONUP => {
            event_sender
                .send(WindowEvent::MouseRelease(MouseButton::Middle))
                .unwrap();
            unsafe { winuser::ReleaseCapture() };
        }
        _ => (),
    }
    // forward to default implementation
    unsafe { winuser::DefWindowProcW(hwnd, umsg, wparam, lparam) }
}
