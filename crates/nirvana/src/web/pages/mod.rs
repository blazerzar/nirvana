mod shell;

use axum::response::Html;
use leptos::prelude::*;

use crate::web::pages::shell::Shell;

/// Renders a full HTML document with `title` and `body`.
fn page(title: &'static str, body: impl IntoView + 'static) -> Html<String> {
    let html = view! {
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>{title}</title>
                <link rel="icon" href="/logo.svg" />
                <link rel="stylesheet" href="/tokens.css" />
                <link rel="stylesheet" href="/app.css" />
                <script defer src="/vendor/alpine.js"></script>
            </head>
            <body>{body}</body>
        </html>
    }
    .to_html();
    Html(format!("<!DOCTYPE html>{html}"))
}

pub(super) async fn home() -> Html<String> {
    page("Nirvana", view! { <Shell /> })
}
