//! Provides a source for window events on Unix platforms.

use super::window::EditorWindowImpl;
use crate::event::WindowEvent;
use crate::platform::EventSourceBackend;

pub(in crate::platform) struct EventSourceImpl {
    connection: xcb::base::Connection,
    size_xy: (i32, i32),
}

impl EventSourceBackend for EventSourceImpl {
    fn new(window: &EditorWindowImpl, size_xy: (i32, i32)) -> Self {
        let connection = unsafe {
            xcb::base::Connection::from_raw_conn(window.connection.as_ref().unwrap().get_raw_conn())
        };

        Self {
            connection,
            size_xy,
        }
    }

    /// The XCB API for getting window events is essentially identical to `vst_window`'s event
    /// polling API.
    fn poll_event(&self) -> Option<WindowEvent> {
        let xcb_event = self.connection.poll_for_event();
        match xcb_event {
            None => None,
            Some(xcb_event) => {
                let r = xcb_event.response_type() & !0x80;
                match r {
                    xcb::MOTION_NOTIFY => {
                        let motion: &xcb::MotionNotifyEvent =
                            unsafe { xcb::cast_event(&xcb_event) };
                        Some(WindowEvent::CursorMovement(
                            motion.event_x() as f32 / self.size_xy.0 as f32,
                            motion.event_y() as f32 / self.size_xy.1 as f32,
                        ))
                    }
                    xcb::BUTTON_PRESS => {
                        let button: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(&xcb_event) };
                        convert_mouse_button_detail(button.detail()).map(WindowEvent::MouseClick)
                    }
                    xcb::BUTTON_RELEASE => {
                        let button: &xcb::ButtonReleaseEvent =
                            unsafe { xcb::cast_event(&xcb_event) };
                        convert_mouse_button_detail(button.detail()).map(WindowEvent::MouseRelease)
                    }
                    _ => {
                        None
                    }
                }
            }
        }
    }
}

fn convert_mouse_button_detail(detail: u8) -> Option<crate::event::MouseButton> {
    use crate::event::MouseButton;
    match detail {
        1 => Some(MouseButton::Left),
        2 => Some(MouseButton::Middle),
        3 => Some(MouseButton::Right),
        _ => None,
    }
}
