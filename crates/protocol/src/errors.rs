use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("invalid job parameters")]
    InvalidJob,
    #[error("invalid technical profile")]
    InvalidProfile,
    #[error("invalid or overflowing counter")]
    InvalidCounter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    Unauthorized,
    Busy,
    Conflict,
    Unavailable,
    Interrupted,
    Cancelled,
    CleanupFailed,
    RestoreFailed,
    Internal,
}
impl ErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            Self::BadRequest => "Некорректные параметры запроса",
            Self::Unauthorized => "Требуется вход",
            Self::Busy => "Уже выполняется другая операция",
            Self::Conflict => "Состояние изменилось; обновите данные",
            Self::Unavailable => "Сервер временно недоступен",
            Self::Interrupted => "Выполнение прервано перезапуском агента",
            Self::Cancelled => "Операция отменена",
            Self::CleanupFailed => "Операция завершилась с ошибкой",
            Self::RestoreFailed => "Не удалось подтвердить восстановление почтовой службы",
            Self::Internal => "Внутренняя ошибка",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PublicError {
    code: ErrorCode,
    message: String,
    correlation_id: Uuid,
}
impl PublicError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message: code.message().into(),
            correlation_id: Uuid::new_v4(),
        }
    }
    pub fn code(&self) -> ErrorCode {
        self.code
    }
}
impl<'de> Deserialize<'de> for PublicError {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            code: ErrorCode,
            message: String,
            correlation_id: Uuid,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.message != wire.code.message() || wire.correlation_id.is_nil() {
            return Err(serde::de::Error::custom("invalid public error"));
        }
        Ok(Self {
            code: wire.code,
            message: wire.message,
            correlation_id: wire.correlation_id,
        })
    }
}
