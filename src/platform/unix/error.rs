use x11rb::rust_connection::{ConnectionError, ReplyError, ReplyOrIdError};

use crate::SetupError;

impl From<ReplyOrIdError> for SetupError {
    fn from(error: ReplyOrIdError) -> Self {
        match error {
            ReplyOrIdError::ConnectionError(conn_err) => {
                SetupError::with_context(conn_err, "display server connection error")
            }
            _ => SetupError::new(error),
        }
    }
}

impl From<ConnectionError> for SetupError {
    fn from(error: ConnectionError) -> Self {
        SetupError::with_context(error, "display server connection error")
    }
}

impl From<ReplyError> for SetupError {
    fn from(error: ReplyError) -> Self {
        match error {
            ReplyError::ConnectionError(conn_err) => {
                SetupError::with_context(conn_err, "display server connection error")
            }
            _ => SetupError::new(error),
        }
    }
}
