//! Provides a source for window events on Unix platforms.

use std::sync::Arc;

use x11rb::connection::Connection;

use super::window::EditorWindowImpl;
use crate::event::WindowEvent;
use crate::platform::EventSourceBackend;

pub(in crate::platform) struct EventSourceImpl {
    connection: Arc<x11rb::xcb_ffi::XCBConnection>,
    size_xy: (i32, i32),
}

impl EventSourceBackend for EventSourceImpl {
    fn new(window: &EditorWindowImpl, size_xy: (i32, i32)) -> anyhow::Result<Self> {
        Ok(Self {
            connection: window.connection.clone(),
            size_xy,
        })
    }

    /// The XCB API for getting window events is essentially identical to `vst_window`'s event
    /// polling API.
    fn poll_event(&self) -> Option<WindowEvent> {
        loop {
            let maybe_event = match self.connection.poll_for_event() {
                Ok(e) => e,
                Err(error) => {
                    log::debug!(
                        "Error: {:#}",
                        anyhow::anyhow!(crate::Error::Other {
                            source: anyhow::anyhow!(error),
                            backend: crate::Backend::X11,
                        })
                        .context("failed to poll for new events")
                    );
                    return None;
                }
            };
            if let Some(event) = maybe_event {
                use x11rb::protocol::Event as X11Event;
                match event {
                    X11Event::MotionNotify(motion_event) => {
                        return Some(WindowEvent::CursorMovement(
                            motion_event.event_x as f32 / self.size_xy.0 as f32,
                            motion_event.event_y as f32 / self.size_xy.1 as f32,
                        ))
                    }
                    X11Event::ButtonPress(button_event) => {
                        if let Some(event) = convert_mouse_button_detail(button_event.detail)
                            .map(WindowEvent::MouseClick)
                        {
                            return Some(event);
                        } else {
                            continue;
                        }
                    }
                    X11Event::ButtonRelease(button_event) => {
                        if let Some(event) = convert_mouse_button_detail(button_event.detail)
                            .map(WindowEvent::MouseRelease)
                        {
                            return Some(event);
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
            } else {
                return None;
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
