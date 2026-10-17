use super::{
    agents::{start_router, TlsAgent},
    CenterFixture,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use hmail_cleaner_protocol::{
    validate_job, JobCommand, JobId, JobPhase, JobSnapshot, ObservedMailState,
    ServiceStateResponse, SummaryDto,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
#[derive(Clone)]
pub struct Control(Arc<Inner>);
struct Inner {
    epoch: uuid::Uuid,
    receipts: Mutex<HashMap<JobId, JobSnapshot>>,
    pub puts: AtomicUsize,
    pub lose_response: AtomicBool,
    pub hide_lookup: AtomicBool,
    pub cancel_first: AtomicBool,
    pub mail: Mutex<ObservedMailState>,
    hold: AtomicBool,
    released: tokio::sync::Semaphore,
    events: tokio::sync::broadcast::Sender<JobSnapshot>,
    subscriptions: AtomicUsize,
}
impl Control {
    pub fn puts(&self) -> usize {
        self.0.puts.load(Ordering::SeqCst)
    }
    pub fn lose_response(&self, value: bool) {
        self.0.lose_response.store(value, Ordering::SeqCst)
    }
    pub fn hide_lookup(&self, value: bool) {
        self.0.hide_lookup.store(value, Ordering::SeqCst)
    }
    pub fn cancel_first(&self, value: bool) {
        self.0.cancel_first.store(value, Ordering::SeqCst)
    }
    pub fn finish(&self, id: JobId, phase: JobPhase) {
        let mut values = self.0.receipts.lock().unwrap();
        let receipt = values.get_mut(&id).unwrap();
        receipt.phase = phase;
        receipt.revision = receipt.revision.next().unwrap();
        receipt.mail_state = ObservedMailState::Running;
        receipt.initial_mail_state = Some(ObservedMailState::Running);
        receipt.observed_at = chrono::Utc::now();
        let _ = self.0.events.send(receipt.clone());
    }
    pub fn receipt(&self, id: JobId) -> Option<JobSnapshot> {
        self.0.receipts.lock().unwrap().get(&id).cloned()
    }
    pub fn mail(&self, value: ObservedMailState) {
        *self.0.mail.lock().unwrap() = value;
    }
    pub fn hold_response(&self) {
        self.0.hold.store(true, Ordering::SeqCst);
    }
    pub fn release_response(&self) {
        self.0.released.add_permits(1);
    }
    pub fn subscriptions(&self) -> usize {
        self.0.subscriptions.load(Ordering::SeqCst)
    }
}
pub async fn start(center: &CenterFixture) -> (TlsAgent, Control) {
    let control = Control(Arc::new(Inner {
        epoch: uuid::Uuid::new_v4(),
        receipts: Mutex::new(HashMap::new()),
        puts: AtomicUsize::new(0),
        lose_response: AtomicBool::new(false),
        hide_lookup: AtomicBool::new(false),
        cancel_first: AtomicBool::new(false),
        mail: Mutex::new(ObservedMailState::Running),
        hold: AtomicBool::new(false),
        released: tokio::sync::Semaphore::new(0),
        events: tokio::sync::broadcast::channel(16).0,
        subscriptions: AtomicUsize::new(0),
    }));
    let app = Router::new()
        .route("/agent/v1/jobs/{id}", get(read).put(submit))
        .route("/agent/v1/jobs/{id}/cancel", axum::routing::put(cancel))
        .route("/agent/v1/service/state", post(service))
        .route("/agent/v1/jobs/{id}/events", get(events))
        .with_state(control.clone());
    let agent = start_router(
        center,
        hmail_cleaner_protocol::AgentId::new(),
        app,
        control.0.epoch,
    )
    .await;
    (agent, control)
}
async fn submit(
    State(control): State<Control>,
    Path(id): Path<JobId>,
    Json(command): Json<JobCommand>,
) -> Result<Json<JobSnapshot>, StatusCode> {
    let job = validate_job(command).map_err(|_| StatusCode::BAD_REQUEST)?;
    if job.command().job_id != id {
        return Err(StatusCode::BAD_REQUEST);
    }
    control.0.puts.fetch_add(1, Ordering::SeqCst);
    let receipt = control
        .0
        .receipts
        .lock()
        .unwrap()
        .entry(id)
        .or_insert_with(|| JobSnapshot {
            job_id: id,
            job: Some(job.public()),
            stream_epoch: control.0.epoch,
            revision: 1.into(),
            phase: JobPhase::Running,
            stage: None,
            summary: SummaryDto::default(),
            mail_state: ObservedMailState::Running,
            initial_mail_state: Some(ObservedMailState::Running),
            error: None,
            observed_at: chrono::Utc::now(),
        })
        .clone();
    if control.0.hold.load(Ordering::SeqCst) {
        control.0.released.acquire().await.unwrap().forget();
    }
    if control.0.lose_response.load(Ordering::SeqCst) {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    } else {
        Ok(Json(receipt))
    }
}
async fn read(
    State(control): State<Control>,
    Path(id): Path<JobId>,
) -> Result<Json<JobSnapshot>, StatusCode> {
    if control.0.hide_lookup.load(Ordering::SeqCst) {
        return Err(StatusCode::NOT_FOUND);
    }
    control.receipt(id).map(Json).ok_or(StatusCode::NOT_FOUND)
}
async fn cancel(State(control): State<Control>, Path(id): Path<JobId>) -> Json<JobSnapshot> {
    let mut receipts = control.0.receipts.lock().unwrap();
    let snapshot = receipts.entry(id).or_insert_with(|| JobSnapshot {
        job_id: id,
        job: None,
        stream_epoch: control.0.epoch,
        revision: 1.into(),
        phase: JobPhase::Cancelled,
        stage: None,
        summary: SummaryDto::default(),
        mail_state: ObservedMailState::Unknown,
        initial_mail_state: None,
        error: None,
        observed_at: chrono::Utc::now(),
    });
    if snapshot.phase.is_active() {
        snapshot.phase = JobPhase::Cancelled;
        snapshot.revision = snapshot.revision.next().unwrap();
    }
    if control.0.cancel_first.load(Ordering::SeqCst) {
        control.0.hide_lookup.store(false, Ordering::SeqCst);
    }
    Json(snapshot.clone())
}
async fn service(State(control): State<Control>) -> Json<ServiceStateResponse> {
    Json(ServiceStateResponse {
        mail_state: *control.0.mail.lock().unwrap(),
        observed_at: chrono::Utc::now(),
    })
}
async fn events(
    State(control): State<Control>,
    Path(id): Path<JobId>,
) -> Result<
    axum::response::sse::Sse<
        impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
    >,
    StatusCode,
> {
    use axum::response::sse::{Event, Sse};
    use futures_util::StreamExt;
    let receiver = control.0.events.subscribe();
    let snapshot = control.receipt(id).ok_or(StatusCode::NOT_FOUND)?;
    control.0.subscriptions.fetch_add(1, Ordering::SeqCst);
    let initial = futures_util::stream::once(async move {
        Ok(Event::default()
            .event("snapshot")
            .json_data(snapshot)
            .unwrap())
    });
    let updates = futures_util::stream::unfold(receiver, move |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(snapshot) if snapshot.job_id == id => {
                    return Some((
                        Ok(Event::default()
                            .event("snapshot")
                            .json_data(snapshot)
                            .unwrap()),
                        receiver,
                    ))
                }
                Ok(_) => (),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    return Some((Ok(Event::default().event("resync").data("{}")), receiver))
                }
                Err(_) => return None,
            }
        }
    });
    Ok(Sse::new(initial.chain(updates)))
}
