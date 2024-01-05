use askama_axum::IntoResponse;
use axum::http::StatusCode;

pub struct TkError(miette::Error);

impl IntoResponse for TkError {
    fn into_response(self) -> askama_axum::Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Internal Server Error: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for TkError
where
    E: Into<miette::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}
