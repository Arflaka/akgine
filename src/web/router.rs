//! A tiny hand-rolled router: no macros, no external routing crate. Good
//! enough for a handful of REST-ish endpoints and keeps the dependency
//! list minimal, per the project's design goals.

use crate::web::error::ApiError;
use crate::web::request::{Method, ParsedRequest};
use crate::web::response::{HttpResponse, resolve};
use std::collections::HashMap;

pub type PathParams = HashMap<String, String>;

type HandlerFn<T> =
    dyn Fn(&T, &ParsedRequest, &PathParams) -> Result<HttpResponse, ApiError> + Send + Sync;

enum Segment {
    Literal(String),
    Param(String),
}

struct Route<T> {
    method: Method,
    segments: Vec<Segment>,
    handler: Box<HandlerFn<T>>,
}

#[derive(Default)]
pub struct Router<T> {
    routes: Vec<Route<T>>,
}

impl<T> Router<T> {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Register a route. `pattern` looks like `/conversations/:id/messages`.
    pub fn add<F>(&mut self, method: Method, pattern: &str, handler: F)
    where
        F: Fn(&T, &ParsedRequest, &PathParams) -> Result<HttpResponse, ApiError>
            + Send
            + Sync
            + 'static,
    {
        let segments = pattern
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| match s.strip_prefix(':') {
                Some(name) => Segment::Param(name.to_string()),
                None => Segment::Literal(s.to_string()),
            })
            .collect();

        self.routes.push(Route {
            method,
            segments,
            handler: Box::new(handler),
        });
    }

    pub fn dispatch(&self, state: &T, req: &ParsedRequest) -> HttpResponse {
        let req_segments: Vec<&str> = req
            .path
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        for route in &self.routes {
            if route.method != req.method || route.segments.len() != req_segments.len() {
                continue;
            }

            let mut params: HashMap<String, String> = PathParams::new();
            let mut matched: bool = true;
            for (segment, actual) in route.segments.iter().zip(req_segments.iter()) {
                match segment {
                    Segment::Literal(lit) if lit == actual => {}
                    Segment::Literal(_) => {
                        matched = false;
                        break;
                    }
                    Segment::Param(name) => {
                        params.insert(name.clone(), (*actual).to_string());
                    }
                }
            }

            if matched {
                return resolve((route.handler)(state, req, &params));
            }
        }

        HttpResponse::not_found()
    }
}

/// Parse a `:id`-style path param as `i64`, or fail with a clean 400.
pub fn param_i64(params: &PathParams, name: &str) -> Result<i64, ApiError> {
    params
        .get(name)
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| ApiError::Validation(format!("path parameter '{name}' must be an integer")))
}
