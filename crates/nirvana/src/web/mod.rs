//! nirvana web and the REST API, served on `127.0.0.1`.

mod api;
mod assets;
mod guard;
mod pages;

use std::io::{self, ErrorKind};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;

use axum::routing::get;
use axum::{Router, middleware};
use nirvana_core::Nirvana;
use tokio::net::TcpListener;
use tracing::info;

use crate::web::guard::Guard;

const ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;

/// Returns the `Host` of the server on `port`.
fn host(port: u16) -> String {
    SocketAddrV4::new(ADDRESS, port).to_string()
}

/// Returns the URL of nirvana web on `port`.
pub(crate) fn url(port: u16) -> String {
    format!("http://{}", host(port))
}

/// Binds `127.0.0.1:port`.
pub(crate) async fn bind(port: u16) -> io::Result<TcpListener> {
    TcpListener::bind((ADDRESS, port))
        .await
        .map_err(|e| match e.kind() {
            ErrorKind::AddrInUse => io::Error::new(
                ErrorKind::AddrInUse,
                format!("port {port} is already in use"),
            ),
            _ => e,
        })
}

/// Serves on `listener` until `shutdown` resolves.
pub(crate) async fn run(
    listener: TcpListener,
    nirvana: Arc<Nirvana>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let port = listener.local_addr()?.port();
    info!(url = url(port), "web server started");
    axum::serve(listener, router(nirvana, port))
        .with_graceful_shutdown(shutdown)
        .await?;
    info!("web server stopped");
    Ok(())
}

fn router(nirvana: Arc<Nirvana>, port: u16) -> Router {
    Router::new()
        .nest("/api", api::router())
        .merge(assets::router())
        .fallback(get(pages::home))
        .layer(middleware::from_fn_with_state(
            Arc::new(Guard::new(port)),
            guard::check,
        ))
        .with_state(nirvana)
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::header::{CONTENT_TYPE, HOST, ORIGIN};
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use tower::ServiceExt;

    use super::*;

    const PORT: u16 = 5167;
    const HOST_VALUE: &str = "127.0.0.1:5167";
    const ORIGIN_VALUE: &str = "http://127.0.0.1:5167";

    async fn send(request: Request<Body>) -> Response {
        let dir = tempfile::tempdir().unwrap();
        let nirvana = Nirvana::open_in(dir.path()).await.unwrap();
        router(Arc::new(nirvana), PORT)
            .oneshot(request)
            .await
            .unwrap()
    }

    async fn get(host: &str, path: &str) -> Response {
        send(
            Request::get(path)
                .header(HOST, host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
    }

    async fn post(origin: Option<&str>, content_type: &str) -> Response {
        let mut request = Request::post("/api/nope")
            .header(HOST, HOST_VALUE)
            .header(CONTENT_TYPE, content_type);
        if let Some(origin) = origin {
            request = request.header(ORIGIN, origin);
        }
        send(request.body(Body::from("{}")).unwrap()).await
    }

    async fn body(response: Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn rejects_foreign_host() {
        for host in [
            "example.com",
            "example.com:5167",
            "localhost:5167",
            "127.0.0.1:8080",
        ] {
            let response = get(host, "/").await;

            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{host}");
        }
    }

    #[tokio::test]
    async fn accepts_local_host() {
        let response = get(HOST_VALUE, "/").await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_post_without_allowed_origin() {
        for origin in [
            None,
            Some("http://example.com"),
            Some("http://localhost:5167"),
            Some("http://127.0.0.1:8080"),
        ] {
            let response = post(origin, "application/json").await;

            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin:?}");
        }
    }

    #[tokio::test]
    async fn rejects_post_without_json() {
        let response = post(Some(ORIGIN_VALUE), "text/plain").await;

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn accepts_json_post_from_local_origin() {
        let response = post(Some(ORIGIN_VALUE), "application/json; charset=utf-8").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn renders_shell_for_any_path() {
        for path in ["/", "/anything"] {
            let response = get(HOST_VALUE, path).await;

            assert_eq!(response.status(), StatusCode::OK, "{path}");
            let html = body(response).await;
            assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
            assert!(html.contains(r#"class="sidebar""#), "{html}");
        }
    }

    #[tokio::test]
    async fn returns_json_404_for_unknown_api_path() {
        let response = get(HOST_VALUE, "/api/nope").await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(body(response).await, r#"{"error":"not found"}"#);
    }

    #[tokio::test]
    async fn returns_info() {
        let response = get(HOST_VALUE, "/api/info").await;

        assert_eq!(response.status(), StatusCode::OK);
        let expected = format!(r#"{{"version":"{}"}}"#, env!("CARGO_PKG_VERSION"));
        assert_eq!(body(response).await, expected);
    }

    #[tokio::test]
    async fn serves_tokens_css() {
        let response = get(HOST_VALUE, "/tokens.css").await;

        assert_eq!(response.headers()[CONTENT_TYPE], "text/css");
        assert!(body(response).await.contains("--bg-default:"));
    }
}
