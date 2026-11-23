use axum::Router;
use axum::http::header::CONTENT_TYPE;
use axum::routing::get;

use crate::tokens::tokens_css;

/// Path, content type and contents of each embedded file.
const FILES: [(&str, &str, &[u8]); 8] = [
    (
        "/app.css",
        "text/css",
        include_bytes!("../../assets/web/app.css"),
    ),
    (
        "/logo.svg",
        "image/svg+xml",
        include_bytes!("../../assets/logo.svg"),
    ),
    (
        "/vendor/alpine.js",
        "text/javascript",
        include_bytes!("../../assets/web/vendor/alpine-3.17.5.min.js"),
    ),
    (
        "/fonts/Inter/Bold.ttf",
        "font/ttf",
        include_bytes!("../../assets/fonts/Inter/Bold.ttf"),
    ),
    (
        "/fonts/JetBrainsMono/Regular.ttf",
        "font/ttf",
        include_bytes!("../../assets/fonts/JetBrainsMono/Regular.ttf"),
    ),
    (
        "/fonts/JetBrainsMono/Medium.ttf",
        "font/ttf",
        include_bytes!("../../assets/fonts/JetBrainsMono/Medium.ttf"),
    ),
    (
        "/fonts/JetBrainsMono/SemiBold.ttf",
        "font/ttf",
        include_bytes!("../../assets/fonts/JetBrainsMono/SemiBold.ttf"),
    ),
    (
        "/fonts/JetBrainsMono/Bold.ttf",
        "font/ttf",
        include_bytes!("../../assets/fonts/JetBrainsMono/Bold.ttf"),
    ),
];

pub(super) fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    let tokens = Router::new().route(
        "/tokens.css",
        get(|| async { ([(CONTENT_TYPE, "text/css")], tokens_css()) }),
    );
    FILES
        .into_iter()
        .fold(tokens, |router, (path, content_type, contents)| {
            router.route(
                path,
                get(move || async move { ([(CONTENT_TYPE, content_type)], contents) }),
            )
        })
}
