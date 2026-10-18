pub(crate) const JOURNAL_FILE: &str = "center.ndjson";
use hmail_cleaner_adapters::journal::RuntimeJournal;
use serde::Serialize;
#[derive(Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub(crate) enum Event {
    ServiceStarting,
    ServiceStopped,
    ServiceFailed,
    RequestFailed {
        correlation_id: uuid::Uuid,
        operation: Operation,
        status: u16,
    },
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Operation {
    Authentication,
    Settings,
    Servers,
    Jobs,
    Agents,
    Enrollment,
    Other,
}
impl Operation {
    fn from_path(path: &str) -> Self {
        match path.split('/').nth(3) {
            Some("auth") => Self::Authentication,
            Some("settings") => Self::Settings,
            Some("servers") => Self::Servers,
            Some("jobs" | "events") => Self::Jobs,
            Some("agents") => Self::Agents,
            Some("enrollment") => Self::Enrollment,
            _ => Self::Other,
        }
    }
}
pub(crate) fn record(journal: Option<&RuntimeJournal>, event: Event) {
    if let Some(journal) = journal {
        let _ = journal.record(&event);
    }
}
pub(crate) async fn request(
    axum::extract::State(state): axum::extract::State<crate::app::AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let operation = Operation::from_path(request.uri().path());
    let response = next.run(request).await;
    if response.status().is_client_error() || response.status().is_server_error() {
        record(
            state.journal.as_ref(),
            Event::RequestFailed {
                correlation_id: uuid::Uuid::new_v4(),
                operation,
                status: response.status().as_u16(),
            },
        );
    }
    response
}
