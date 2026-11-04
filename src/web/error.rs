/*! `ApiError` is the error type that export all problem with the http service */
#[cfg(feature = "database")]
#[cfg_attr(docsrs, doc(cfg(feature = "database")))]
use crate::database::DbError;
use std::fmt;

#[derive(Debug)]
pub enum ApiError {
    /** The requested resource does not exist. -> HTTP 404. */
    NotFound(String),
    /** The request body/parameters failed validation. -> HTTP 400. */
    Validation(String),
    /** The request conflicts with existing data. -> HTTP 409. */
    Conflict(String),
    /** Anything else (I/O, SQL, ...). -> HTTP 500. */
    Internal(String),
}

impl ApiError {
    /**
     * Retrun the code http to send to the client
     * - `ApiError::NotFound => 404`
     * - `ApiError::Validation => 400`
     * - `ApiError::Conflict => 409`
     * - `ApiError::Internal => 500`
     */
    pub fn status_code(&self) -> u16 {
        match self {
            ApiError::NotFound(_) => 404,
            ApiError::Validation(_) => 400,
            ApiError::Conflict(_) => 409,
            ApiError::Internal(_) => 500,
        }
    }

    /** Message to send back to the client. */
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

#[cfg(feature = "database")]
#[cfg_attr(docsrs, doc(cfg(feature = "database")))]
/** Translate a raw `akgine::DbError` into an `ApiError`. */
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
