/*!
 * make one thread by worker to handle all request
 */

use crate::router::{PathParams, Router};
use crate::web::error::ApiError;
use crate::web::request::{Method, ParsedRequest};
use crate::web::response::{HttpResponse, resolve};
use std::sync::Arc;
use tiny_http::Header;
use tiny_http::Response;
use tiny_http::Server;

/** Define the exact function signature required for all route handlers. */
pub type HandlerFn<T> =
    dyn Fn(&T, &ParsedRequest, &PathParams) -> Result<HttpResponse, ApiError> + Send + Sync;

pub fn run<T: Clone + Send + 'static>(
    bind_addr: &str,
    worker_threads: usize,
    state: T,
    router: Router<Method, Arc<HandlerFn<T>>>,
) {
    let server: Arc<Server> = match Server::http(bind_addr) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("failed to bind {bind_addr}: {e}");
            std::process::exit(1);
        }
    };
    let router: Arc<Router<Method, Arc<HandlerFn<T>>>> = Arc::new(router);

    println!("chat-backend listening on http://{bind_addr} ({worker_threads} workers)");

    let handles: Vec<_> = (0..worker_threads.max(1))
        .map(|_| {
            let server = server.clone();
            let state = state.clone();
            let router = router.clone();
            std::thread::spawn(move || worker_loop(server, state, router))
        })
        .collect();

    for handle in handles {
        let _ = handle.join();
    }
}

fn worker_loop<T: Clone>(
    server: Arc<Server>,
    state: T,
    router: Arc<Router<Method, Arc<HandlerFn<T>>>>,
) {
    loop {
        match server.recv() {
            Ok(mut request) => {
                let parsed: ParsedRequest = ParsedRequest::from_tiny_http(&mut request);
                let routerResponse: Option<(
                    Arc<HandlerFn<T>>,
                    std::collections::HashMap<String, String>,
                )> = router.dispatch(&parsed.path, parsed.method);
                let response: HttpResponse = match routerResponse {
                    Some((handler, params)) => resolve((handler)(&state, &parsed, &params)),
                    None => HttpResponse::not_found(),
                };

                let http_response: Response<std::io::Cursor<Vec<u8>>> =
                    Response::from_data(response.body)
                        .with_status_code(response.status)
                        // .with_status_code(200)
                        .with_header(
                            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                                .expect("static header is always valid"),
                        );

                if let Err(e) = request.respond(http_response) {
                    eprintln!("failed to write response: {e}");
                }
            }
            Err(e) => eprintln!("HTTP server error: {e}"),
        }
    }
}
