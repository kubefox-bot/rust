use crate::execution::AgentError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use hmail_cleaner_protocol::{ErrorCode, PublicError};

pub(super) struct ApiError(pub StatusCode, pub ErrorCode);
impl ApiError {
    pub fn unavailable() -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable)
    }
    pub fn bad_request() -> Self {
        Self(StatusCode::BAD_REQUEST, ErrorCode::BadRequest)
    }
    pub fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, ErrorCode::BadRequest)
    }
}
impl From<AgentError> for ApiError {
    fn from(error: AgentError) -> Self {
        match error {
            AgentError::Invalid => Self::bad_request(),
            AgentError::Busy => Self(StatusCode::CONFLICT, ErrorCode::Busy),
            AgentError::Conflict => Self(StatusCode::CONFLICT, ErrorCode::Conflict),
            AgentError::Unavailable | AgentError::Storage(_) | AgentError::Transport(_) => {
                Self::unavailable()
            }
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(PublicError::new(self.1))).into_response()
    }
}
