use uuid::Uuid;

use crate::{host::error::HostError, session::error::SessionError};

#[derive(Debug)]
pub enum AppNotification {
    HostError(HostError),
    SessionError {
        session_id: Uuid,
        error: SessionError,
    },
    /// General error notification.
    Error(String),
    /// General warning notification.
    Warning(String),
    /// General info notification.
    Info(String),
}
