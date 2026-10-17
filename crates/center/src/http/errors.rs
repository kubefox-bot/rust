use crate::jobs::SubmitError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use hmail_cleaner_protocol::{ErrorCode, PublicError};
pub(crate) struct ApiError(StatusCode, ErrorCode);
impl ApiError {
    pub(crate) fn busy() -> Self {
        Self(StatusCode::CONFLICT, ErrorCode::Busy)
    }
    pub(crate) fn conflict() -> Self {
        Self(StatusCode::CONFLICT, ErrorCode::Conflict)
    }
    pub fn bad_request() -> Self {
        Self(StatusCode::BAD_REQUEST, ErrorCode::BadRequest)
    }
    pub fn unavailable() -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable)
    }
}
impl From<StatusCode> for ApiError {
    fn from(status: StatusCode) -> Self {
        Self(
            status,
            if status == StatusCode::UNAUTHORIZED {
                ErrorCode::Unauthorized
            } else {
                ErrorCode::BadRequest
            },
        )
    }
}
impl From<crate::profiles::ProfileError> for ApiError {
    fn from(error: crate::profiles::ProfileError) -> Self {
        use crate::profiles::ProfileError;
        match error {
            ProfileError::Invalid => Self::bad_request(),
            ProfileError::Conflict => Self(StatusCode::CONFLICT, ErrorCode::Conflict),
            ProfileError::NotFound => Self(StatusCode::NOT_FOUND, ErrorCode::BadRequest),
            ProfileError::Storage => Self::unavailable(),
            ProfileError::Unauthorized => Self(StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized),
        }
    }
}
impl From<crate::agents::AgentError> for ApiError {
    fn from(error: crate::agents::AgentError) -> Self {
        use crate::agents::AgentError;
        match error {
            AgentError::Unavailable => Self::unavailable(),
            AgentError::Protocol => Self(StatusCode::BAD_GATEWAY, ErrorCode::Unavailable),
            AgentError::Rejected => Self::bad_request(),
            AgentError::NotFound => Self(StatusCode::NOT_FOUND, ErrorCode::BadRequest),
            AgentError::Conflict => Self(StatusCode::CONFLICT, ErrorCode::Conflict),
            AgentError::Revoked => Self(StatusCode::UNAUTHORIZED, ErrorCode::Unauthorized),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(PublicError::new(self.1))).into_response()
    }
}

impl From<SubmitError> for ApiError {
    fn from(error: SubmitError) -> Self {
        use axum::http::StatusCode;
        match error {
            SubmitError::Busy => ApiError::busy(),
            SubmitError::Conflict => ApiError::conflict(),
            SubmitError::Invalid => ApiError::bad_request(),
            SubmitError::Unauthorized => StatusCode::UNAUTHORIZED.into(),
            SubmitError::NotFound => StatusCode::NOT_FOUND.into(),
            SubmitError::Storage | SubmitError::Protocol => ApiError::unavailable(),
        }
    }
}
