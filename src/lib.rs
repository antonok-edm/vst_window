//! `vst_window` provides a cross-platform API for implementing VST plugin editor windows.
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "macos",
    target_os = "windows"
)))]
compile_error!("This target_os is not supported.");

mod event;
mod platform;

use std::fmt::Display;

pub use event::{MouseButton, WindowEvent};
pub use platform::{setup, EditorWindow};

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
const CURRENT_BACKEND: Backend = Backend::X11;
#[cfg(target_os = "macos")]
const CURRENT_BACKEND: Backend = Backend::X11;
#[cfg(target_os = "windows")]
const CURRENT_BACKEND: Backend = Backend::X11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Backend {
    AppKit,
    Win32,
    X11,
}

impl Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Backend::*;
        let name = match *self {
            AppKit => "AppKit",
            Win32 => "Win32",
            X11 => "X11",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug)]
pub struct SetupError {
    source: Box<dyn std::error::Error + Send + Sync + 'static>,
    backend: Backend,
    context: Option<&'static str>,
}

impl SetupError {
    pub fn backend(&self) -> Backend {
        self.backend
    }

    #[allow(dead_code)] // might not be used by all platform implementations
    pub(crate) fn with_context<E>(source: E, context: &'static str) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::with_context_boxed(Box::new(source), context)
    }

    #[allow(dead_code)] // might not be used by all platform implementations
    pub(crate) fn with_context_boxed(
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
        context: &'static str,
    ) -> Self {
        Self {
            source,
            backend: CURRENT_BACKEND,
            context: Some(context),
        }
    }

    #[allow(dead_code)] // might not be used by all platform implementations
    pub(crate) fn new<E>(source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::new_boxed(Box::new(source))
    }

    #[allow(dead_code)] // might not be used by all platform implementations
    pub(crate) fn new_boxed(source: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self {
        Self {
            source,
            backend: CURRENT_BACKEND,
            context: None,
        }
    }
}

impl Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(context) = self.context {
            write!(f, "{} ({})", context, self.backend)
        } else {
            write!(f, "platform error ({})", self.backend)
        }
    }
}

impl std::error::Error for SetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

#[derive(Debug)]
// Determined on a best-effort basis to aid debugging
pub(crate) struct InvalidParentError(String);

impl InvalidParentError {
    pub fn new(parent: *mut std::os::raw::c_void) -> Self {
        if parent.is_null() {
            InvalidParentError("null".into())
        } else {
            InvalidParentError(format!("{:?}", parent))
        }
    }
}

impl Display for InvalidParentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "supplied parent pointer ({}) is not valid for this platform",
            self.0
        )
    }
}

impl std::error::Error for InvalidParentError {}

#[derive(Debug)]
// Determined on a best-effort basis to aid debugging
pub(crate) struct InvalidSizeError((i32, i32));

impl Display for InvalidSizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "supplied window size (width: {}, height: {}) is not valid for this platform",
            self.0 .0, self.0 .1
        )
    }
}

impl std::error::Error for InvalidSizeError {}

pub(crate) struct ErrorChainPrinter<E>(E);

impl<E: std::error::Error> Display for ErrorChainPrinter<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;

        let mut maybe_error = self.0.source();
        while let Some(error) = maybe_error {
            write!(f, ": {}", error)?;
            maybe_error = error.source();
        }

        Ok(())
    }
}
