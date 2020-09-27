//! Provides a source for window events on Windows platforms.

use std::sync::mpsc::{channel, Receiver, Sender};

use winapi::{
    shared::{minwindef, windef},
    um::winuser,
};

use crate::event::{MouseButton, WindowEvent};
use crate::platform::EditorWindowImpl;
use crate::platform::EventSourceBackend;

pub(in crate::platform) struct EventSourceImpl {
    hwnd: windef::HWND,
    incoming_window_events: Receiver<WindowEvent>,
}

impl EventSourceBackend for EventSourceImpl {
    /// Window events must be received in the "window process" defined by the window's class, in
    /// its own main thread. However, the inversion of control flow in the VST API means that we
    /// can't run the windowing logic in the window process. Instead, we just use it to forward the
    /// events over a channel so that they can be polled lazily from the editor's `idle` function.
    /// The channel sender is heap-allocated, and its pointer is stored as extra "user data"
    /// associated with the HWND.
    fn new(window: &EditorWindowImpl, _size_xy: (i32, i32)) -> Self {
        let (event_sender, incoming_window_events) = channel();
        let event_sender_ptr = Box::into_raw(Box::new(event_sender));
        unsafe {
            winuser::SetWindowLongPtrW(
                window.hwnd,
                winuser::GWLP_USERDATA,
                event_sender_ptr as winapi::shared::basetsd::LONG_PTR,
            )
        };
        Self {
            hwnd: window.hwnd,
            incoming_window_events,
        }
    }

    fn poll_event(&self) -> Option<WindowEvent> {
        self.incoming_window_events.try_recv().ok()
    }
}

impl Drop for EventSourceImpl {
    fn drop(&mut self) {
        unsafe {
            let event_sender_ptr = winuser::GetWindowLongPtrW(self.hwnd, winuser::GWLP_USERDATA);
            Box::from_raw(event_sender_ptr as *mut Sender<WindowEvent>);
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
    let event_sender = {
        let event_sender_ptr = winuser::GetWindowLongPtrW(hwnd, winuser::GWLP_USERDATA);
        &mut *(event_sender_ptr as *mut Sender<WindowEvent>)
    };

    match umsg {
        winuser::WM_GETDLGCODE => winuser::DLGC_WANTALLKEYS,
        winuser::WM_MOUSEMOVE => {
            let mut window_bounds: windef::RECT = std::mem::zeroed();
            winuser::GetWindowRect(hwnd, &mut window_bounds as *mut windef::RECT);
            let x_px = winapi::shared::windowsx::GET_X_LPARAM(lparam);
            let y_px = winapi::shared::windowsx::GET_Y_LPARAM(lparam);
            let x = (x_px as f32) / ((window_bounds.right - window_bounds.left) as f32);
            let y = (y_px as f32) / ((window_bounds.bottom - window_bounds.top) as f32);
            event_sender
                .send(WindowEvent::CursorMovement(x, y))
                .unwrap();
            winuser::DefWindowProcW(hwnd, umsg, wparam, lparam)
        }
        winuser::WM_LBUTTONDOWN => {
            event_sender
                .send(WindowEvent::MouseClick(MouseButton::Left))
                .unwrap();
            winapi::um::winuser::SetCapture(hwnd);
            winuser::DefWindowProcW(hwnd, umsg, wparam, lparam)
        }
        winuser::WM_LBUTTONUP => {
            event_sender
                .send(WindowEvent::MouseRelease(MouseButton::Left))
                .unwrap();
            winapi::um::winuser::ReleaseCapture();
            winuser::DefWindowProcW(hwnd, umsg, wparam, lparam)
        }
        winuser::WM_RBUTTONDOWN => {
            event_sender
                .send(WindowEvent::MouseClick(MouseButton::Right))
                .unwrap();
            winapi::um::winuser::SetCapture(hwnd);
            winuser::DefWindowProcW(hwnd, umsg, wparam, lparam)
        }
        winuser::WM_RBUTTONUP => {
            event_sender
                .send(WindowEvent::MouseRelease(MouseButton::Right))
                .unwrap();
            winapi::um::winuser::ReleaseCapture();
            winuser::DefWindowProcW(hwnd, umsg, wparam, lparam)
        }
        winuser::WM_MBUTTONDOWN => {
            event_sender
                .send(WindowEvent::MouseClick(MouseButton::Middle))
                .unwrap();
            winapi::um::winuser::SetCapture(hwnd);
            winuser::DefWindowProcW(hwnd, umsg, wparam, lparam)
        }
        winuser::WM_MBUTTONUP => {
            event_sender
                .send(WindowEvent::MouseRelease(MouseButton::Middle))
                .unwrap();
            winapi::um::winuser::ReleaseCapture();
            winuser::DefWindowProcW(hwnd, umsg, wparam, lparam)
        }
        _ => winuser::DefWindowProcW(hwnd, umsg, wparam, lparam),
    }
}
