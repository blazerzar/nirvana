use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use nirvana_core::{Nirvana, NirvanaError};
use serde::Serialize;

pub(super) fn router() -> Router<Arc<Nirvana>> {
    Router::new().route("/info", get(info)).fallback(not_found)
}

/// An API error, sent as its status and `{ "error": "<message>" }`.
pub(super) struct Error {
    status: StatusCode,
    message: String,
}

impl Error {
    pub(super) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<NirvanaError> for Error {
    fn from(error: NirvanaError) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: self.message,
        };
        (self.status, Json(body)).into_response()
    }
}

async fn not_found() -> Error {
    Error::new(StatusCode::NOT_FOUND, "not found")
}

#[derive(Serialize)]
struct Info {
    version: String,
}

async fn info(State(nirvana): State<Arc<Nirvana>>) -> Result<Json<Info>, Error> {
    let info = nirvana.info().await?;
    Ok(Json(Info {
        version: info.version,
    }))
}
