//! A small, transport-agnostic request type. Handlers and the router only
//! ever see `Method`/`ParsedRequest` — never `tiny_http` types — so
//! swapping the HTTP crate later (e.g. for hyper) only means rewriting
//! `http::server`.

use crate::web::error::ApiError;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Other,
}

impl From<&tiny_http::Method> for Method {
    fn from(m: &tiny_http::Method) -> Self {
        match m {
            tiny_http::Method::Get => Method::Get,
            tiny_http::Method::Post => Method::Post,
            tiny_http::Method::Put => Method::Put,
            tiny_http::Method::Delete => Method::Delete,
            tiny_http::Method::Patch => Method::Patch,
            _ => Method::Other,
        }
    }
}

pub struct ParsedRequest {
    pub method: Method,
    /// Path only (no query string), percent-decoded, without a trailing slash.
    pub path: String,
    pub query: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl ParsedRequest {
    /// Read method/URL/body off a live tiny_http request. The only place in
    /// the codebase that touches `tiny_http` types directly besides `server.rs`.
    pub fn from_tiny_http(request: &mut tiny_http::Request) -> Self {
        let method = Method::from(request.method());

        let (raw_path, raw_query) = match request.url().split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (request.url(), None),
        };
        let path = percent_decode(raw_path.trim_end_matches('/'));
        let query = raw_query.map(parse_query).unwrap_or_default();

        let mut body = Vec::new();
        // A malformed/absent body is not fatal here: handlers that need a
        // body will fail their own `json_body()` call with a clear 400.
        let _ = request.as_reader().read_to_end(&mut body);

        Self {
            method,
            path,
            query,
            body,
        }
    }

    /// Parse the JSON body into `T`, returning a clean 400 on failure
    /// instead of letting a panic or an opaque error reach the client.
    pub fn json_body<T: DeserializeOwned>(&self) -> Result<T, ApiError> {
        serde_json::from_slice(&self.body)
            .map_err(|e| ApiError::Validation(format!("invalid JSON body: {e}")))
    }

    pub fn query_i64(&self, key: &str) -> Option<i64> {
        self.query.get(key)?.parse().ok()
    }
}

fn parse_query(raw: &str) -> HashMap<String, String> {
    raw.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (percent_decode(k), percent_decode(v)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

/// Minimal `application/x-www-form-urlencoded`-style decoder (`%XX` and `+`).
/// Kept in-house rather than pulling in a crate just for this.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
