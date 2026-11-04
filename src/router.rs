use std::collections::HashMap;

pub type PathParams = HashMap<String, String>;

enum Segment {
    Literal(String),
    Param(String),
}

struct Route<Method: PartialEq, Response: Clone> {
    method: Method,
    segments: Vec<Segment>,
    response: Response,
}

#[derive(Default)]
pub struct Router<Method: PartialEq, Response: Clone> {
    routes: Vec<Route<Method, Response>>,
}

impl<Method: PartialEq, Response: Clone> Router<Method, Response> {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Register a route. `pattern` looks like `/conversations/:id/messages`.
    pub fn add(&mut self, method: Method, pattern: &str, response: Response) {
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
            response,
        });
    }

    pub fn dispatch(&self, path: String, method: Method) -> Option<(Response, PathParams)> {
        let req_segments: Vec<&str> = path
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        for route in &self.routes {
            if (route.method != method || route.segments.len() != req_segments.len()) {
                continue;
            }

            let mut params: PathParams = PathParams::new();

            let matched: bool = route
                .segments
                .iter()
                .zip(req_segments.iter())
                .try_for_each(|(segment, actual)| {
                    match segment {
                        Segment::Literal(lit) if lit == *actual => Ok(()),
                        Segment::Literal(_) => Err(()), // Stops the loop immediately on mismatch
                        Segment::Param(name) => {
                            params.insert(name.clone(), (*actual).to_string());
                            Ok(())
                        }
                    }
                })
                .is_ok();

            if (matched) {
                return Some((route.response.clone(), params));
            }
        }
        return None;
    }
}

// Parse a `:id`-style path param as `i64`, or fail with a clean 400.
// pub fn param_i64(params: &PathParams, name: &str) -> Result<i64, RouterError> {
//     params
//         .get(name)
//         .and_then(|v| v.parse().ok())
//         .ok_or_else(|| {
//             RouterError::Validation(format!("path parameter '{name}' must be an integer"))
//         })
// }
// }
