//! Blocking, thread-per-worker HTTP server on top of `tiny_http`.
//!
//! There is no async runtime here, so there is nothing to accidentally
//! block: every worker thread does its own I/O (reading the request,
//! talking to `akgine`, writing the response) on its own OS thread.
//! `akgine::database::DataBase` serializes actual SQL behind one mutex
//! regardless, so this mostly buys concurrency for request parsing/JSON
//! encoding rather than for the database itself.

use crate::web::request::ParsedRequest;
use crate::web::router::Router;
use std::sync::Arc;

pub fn run<T: Clone + Send + 'static>(
    bind_addr: &str,
    worker_threads: usize,
    state: T,
    router: Router<T>,
) {
    let server = match tiny_http::Server::http(bind_addr) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("failed to bind {bind_addr}: {e}");
            std::process::exit(1);
        }
    };
    let router = Arc::new(router);

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

fn worker_loop<T>(server: Arc<tiny_http::Server>, state: T, router: Arc<Router<T>>) {
    loop {
        match server.recv() {
            Ok(mut request) => {
                let parsed: ParsedRequest = ParsedRequest::from_tiny_http(&mut request);
                let response: super::response::HttpResponse = router.dispatch(&state, &parsed);

                let http_response: tiny_http::Response<std::io::Cursor<Vec<u8>>> =
                    tiny_http::Response::from_data(response.body)
                        .with_status_code(response.status)
                        // .with_status_code(200)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                &b"Content-Type"[..],
                                &b"application/json"[..],
                            )
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
