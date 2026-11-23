use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::{CONTENT_TYPE, HOST, ORIGIN};
use axum::http::{HeaderName, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::web::api::Error;

/// The `Host` and `Origin` values the server accepts.
pub(super) struct Guard {
    host: String,
    origin: String,
}

impl Guard {
    pub(super) fn new(port: u16) -> Self {
        let host = format!("127.0.0.1:{port}");
        let origin = format!("http://{host}");
        Self { host, origin }
    }

    /// Accepts a request with the allowed `Host`. A request other than `GET`
    /// or `HEAD` also needs the allowed `Origin` and a JSON content type.
    fn verify(&self, request: &Request) -> Result<(), Error> {
        let header = |name: HeaderName| {
            request
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
        };

        if header(HOST) != Some(self.host.as_str()) {
            return Err(Error::new(StatusCode::FORBIDDEN, "host not allowed"));
        }
        if matches!(*request.method(), Method::GET | Method::HEAD) {
            return Ok(());
        }
        if header(ORIGIN) != Some(self.origin.as_str()) {
            return Err(Error::new(StatusCode::FORBIDDEN, "origin not allowed"));
        }
        let is_json = header(CONTENT_TYPE)
            .and_then(|value| value.split(';').next())
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"));
        if !is_json {
            return Err(Error::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "content type must be application/json",
            ));
        }
        Ok(())
    }
}

pub(super) async fn check(
    State(guard): State<Arc<Guard>>,
    request: Request,
    next: Next,
) -> Response {
    match guard.verify(&request) {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}
