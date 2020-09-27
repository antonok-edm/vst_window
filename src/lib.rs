//! `vst_window` provides a cross-platform API for implementing VST plugin editor windows.

mod event;
mod platform;

pub use event::{MouseButton, WindowEvent};
pub use platform::{setup, EditorWindow, EventSource};
