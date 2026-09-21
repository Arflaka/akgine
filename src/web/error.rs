//! `ApiError` is the single error type that flows from the repository layer
//! up to the HTTP layer. Keeping it here (rather than letting `akgine::DbError`
//! leak into handlers) is what lets us swap the database backend later
//! without touching `handlers/` at all.

use crate::database::DbError;
use std::fmt;

#[derive(Debug)]
pub enum ApiError {
    /// The requested resource does not exist. -> HTTP 404.
    NotFound(String),
    /// The request body/parameters failed validation. -> HTTP 400.
    Validation(String),
    /// The request conflicts with existing state (duplicate username, etc.). -> HTTP 409.
    Conflict(String),
    /// Anything else (I/O, SQL, ...). -> HTTP 500. The detailed message is
    /// logged server-side but not echoed to the client.
    Internal(String),
}

impl ApiError {
    pub fn status_code(&self) -> u16 {
        match self {
            ApiError::NotFound(_) => 404,
            ApiError::Validation(_) => 400,
            ApiError::Conflict(_) => 409,
            ApiError::Internal(_) => 500,
        }
    }

    /// Message safe to send back to the client (internal errors are redacted).
    pub fn public_message(&self) -> String {
        match self {
            ApiError::NotFound(m) | ApiError::Validation(m) | ApiError::Conflict(m) => m.clone(),
            ApiError::Internal(_) => "internal server error".to_string(),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::NotFound(m) => write!(f, "not found: {m}"),
            ApiError::Validation(m) => write!(f, "validation: {m}"),
            ApiError::Conflict(m) => write!(f, "conflict: {m}"),
            ApiError::Internal(m) => write!(f, "internal: {m}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// Translate a raw `akgine::DbError` into an `ApiError`.
///
/// IMPORTANT: the installed `akgine` version can leave its shared connection
/// stuck in an open transaction after a failed `insert()` (see the module
/// docs in `repositories::user_repository` for the full explanation and the
/// pre-validation mitigation used throughout `repositories/`). That specific
/// SQLite message is detected here and turned into a clearly labeled
/// internal error instead of a generic one, so it is easy to spot in logs.
impl From<DbError> for ApiError {
    fn from(e: DbError) -> Self {
        match &e {
            DbError::NotFound => ApiError::NotFound("record not found".into()),
            DbError::Validation(msg) => ApiError::Validation(msg.clone()),
            DbError::Sql(sql_err)
                if sql_err
                    .to_string()
                    .contains("transaction within a transaction") =>
            {
                ApiError::Internal(format!(
                    "database connection is stuck after a failed insert (known akgine issue, \
                     process restart required) - {sql_err}"
                ))
            }
            other => ApiError::Internal(other.to_string()),
        }
    }
}
