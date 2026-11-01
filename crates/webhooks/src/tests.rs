use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode, header};
use kellnr_http::test_support::TestServer;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

/// A request received by a [`TestListener`].
pub(crate) struct Received {
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// Webhook target on a free port that answers every request the same way and
/// reports each received request on `rx`.
pub(crate) struct TestListener {
    server: TestServer,
    pub rx: UnboundedReceiver<Received>,
}

impl TestListener {
    pub(crate) fn url(&self) -> String {
        self.server.url("/")
    }

    pub(crate) fn port(&self) -> u16 {
        self.server.addr().port()
    }
}

pub(crate) async fn get_test_listener(status_code: u16) -> TestListener {
    get_test_listener_with_response(status_code, HeaderMap::new(), String::new()).await
}

pub(crate) async fn get_redirect_listener(location: &str) -> TestListener {
    let mut headers = HeaderMap::new();
    headers.insert(header::LOCATION, location.parse().unwrap());
    get_test_listener_with_response(302, headers, String::new()).await
}

pub(crate) async fn get_test_listener_with_response(
    status_code: u16,
    headers: HeaderMap,
    body: String,
) -> TestListener {
    let status = StatusCode::from_u16(status_code).unwrap();
    let (tx, rx) = unbounded_channel();
    let router = Router::new().fallback(move |request_headers: HeaderMap, request_body: Bytes| {
        let _ = tx.send(Received {
            headers: request_headers,
            body: request_body,
        });
        let response = (status, headers.clone(), body.clone());
        async move { response }
    });
    TestListener {
        server: TestServer::start(router).await,
        rx,
    }
}

/// Webhook target on a free port that answers every request with 200 after
/// `delay`, handling requests in parallel.
pub(crate) async fn get_slow_listener(delay: Duration) -> TestServer {
    TestServer::start(Router::new().fallback(move || async move {
        tokio::time::sleep(delay).await;
        StatusCode::OK
    }))
    .await
}
