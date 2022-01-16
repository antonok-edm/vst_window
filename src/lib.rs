//! `vst_window` provides a cross-platform API for implementing VST plugin editor windows.
#![deny(unsafe_op_in_unsafe_fn)]

mod event;
mod platform;

use std::fmt::Display;

pub use event::{MouseButton, WindowEvent};
pub use platform::{setup, EditorWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Backend {
    Cocoa,
    WinApi,
    X11,
}

impl Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Backend::*;
        let name = match *self {
            Cocoa => "Cocoa",
            WinApi => "Win32",
            X11 => "X11",
        };
        write!(f, "{}", name)
    }
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("requested window size (x, y: {requested_size_xy:?}) out of range ({limits:?})")]
    InvalidWindowSize {
        requested_size_xy: (i32, i32),
        limits: std::ops::Range<i32>,
    },
    #[error("unclassified error ({backend})")]
    Other {
        source: anyhow::Error,
        backend: Backend,
    },
}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        if let Some(err) = error.downcast_ref::<Self>() {
            match err {
                Self::InvalidWindowSize {
                    requested_size_xy,
                    limits,
                } => {
                    return Self::InvalidWindowSize {
                        requested_size_xy: *requested_size_xy,
                        limits: limits.clone(),
                    }
                }
                &Self::Other { backend, .. } => {
                    return Self::Other {
                        source: error,
                        backend,
                    }
                }
            };
        }

        // X11
        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            use x11rb::errors::*;
            if error.downcast_ref::<ConnectionError>().is_some() {
                return Self::Other {
                    source: error,
                    backend: Backend::X11,
                };
            }
            if error.downcast_ref::<ReplyOrIdError>().is_some() {
                return Self::Other {
                    source: error,
                    backend: Backend::X11,
                };
            }
            if error.downcast_ref::<ConnectError>().is_some() {
                return Self::Other {
                    source: error,
                    backend: Backend::X11,
                };
            }
        }

        unreachable!("Unhandled internal error type")
    }
}

pub type Result<T> = std::result::Result<T, crate::Error>;
