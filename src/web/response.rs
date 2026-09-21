use crate::web::error::ApiError;
use serde::Serialize;

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json<T: Serialize>(status: u16, value: &T) -> Self {
        let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
        Self { status, body }
    }

    pub fn status_only(status: u16) -> Self {
        Self {
            status,
            body: Vec::new(),
        }
    }

    pub fn not_found() -> Self {
        #[derive(Serialize)]
        struct Body<'a> {
            error: &'a str,
        }
        Self::json(404, &Body { error: "not found" })
    }
}

impl From<&ApiError> for HttpResponse {
    fn from(e: &ApiError) -> Self {
        #[derive(Serialize)]
        struct Body {
            error: String,
        }
        // Internal errors are logged server-side with full detail but never
        // echoed to the client (see `ApiError::public_message`).
        if e.status_code() == 500 {
            eprintln!("internal error: {e}");
        }
        Self::json(
            e.status_code(),
            &Body {
                error: e.public_message(),
            },
        )
    }
}

/// Handlers return `Result<HttpResponse, ApiError>`; this turns that into
/// the single `HttpResponse` the router sends back.
pub fn resolve(result: Result<HttpResponse, ApiError>) -> HttpResponse {
    match result {
        Ok(resp) => resp,
        Err(e) => HttpResponse::from(&e),
    }
}
