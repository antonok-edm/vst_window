//! Cross-platform type abstractions over low-level platform-specific window events.

/// Represents an interaction with an editor window.
#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    /// XY coordinates. Each coordinate is based in the range [0, 1], scaled to the bounds of the
    /// window. Origin is at the top-left. The coordinates could be outside of the range if the
    /// cursor is outside of the window.
    CursorMovement(f32, f32),
    MouseClick(MouseButton),
    MouseRelease(MouseButton),
}

/// Represents one of the buttons on a mouse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}
