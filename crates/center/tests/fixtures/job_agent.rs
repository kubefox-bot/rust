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
    cancels: AtomicUsize,
    restore_on_cancel: AtomicBool,
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
    pub fn cancels(&self) -> usize {
        self.0.cancels.load(Ordering::SeqCst)
    }
    pub fn restore_on_cancel(&self) {
        self.0.restore_on_cancel.store(true, Ordering::SeqCst);
    }
    pub fn latest(&self) -> Option<JobSnapshot> {
        self.0
            .receipts
            .lock()
            .unwrap()
            .values()
            .max_by_key(|value| value.observed_at)
            .cloned()
    }
    pub fn progress(&self, id: JobId, phase: JobPhase) {
        let mut values = self.0.receipts.lock().unwrap();
        let receipt = values.get_mut(&id).unwrap();
        receipt.phase = phase;
        receipt.revision = receipt.revision.next().unwrap();
        receipt.initial_mail_state = Some(ObservedMailState::Running);
        receipt.mail_state = if phase == JobPhase::Restoring {
            ObservedMailState::Restoring
        } else {
            ObservedMailState::Running
        };
        receipt.summary.selected_files = 12.into();
        receipt.summary.selected_bytes = 50_331_648.into();
        if phase == JobPhase::Completed
            && receipt
                .job
                .as_ref()
                .is_some_and(|job| job.operation == hmail_cleaner_protocol::Operation::Clean)
        {
            receipt.summary.removed_files = 10.into();
            receipt.summary.absent_files = 2.into();
            receipt.summary.removed_bytes = 41_943_040.into();
        }
        receipt.observed_at = chrono::Utc::now();
        let _ = self.0.events.send(receipt.clone());
    }
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
        cancels: AtomicUsize::new(0),
        restore_on_cancel: AtomicBool::new(false),
        lose_response: AtomicBool::new(false),
        hide_lookup: AtomicBool::new(false),
        cancel_first: AtomicBool::new(false),
        mail: Mutex::new(ObservedMailState::Running),
        hold: AtomicBool::new(false),
        released: tokio::sync::Semaphore::new(0),
        events: tokio::sync::broadcast::channel(16).0,
        subscriptions: AtomicUsize::new(0),
    }));
    let agent_id = hmail_cleaner_protocol::AgentId::new();
    let app = Router::new()
        .route(
            "/agent/v1/resolve",
            post(
                move |Json(request): Json<hmail_cleaner_protocol::ResolveRequest>| async move {
                    if request.agent_id != agent_id
                        || request.protocol_version != hmail_cleaner_protocol::PROTOCOL_VERSION
                    {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                    hmail_cleaner_agent::cutoff::resolve(
                        &chrono::FixedOffset::east_opt(10800).unwrap(),
                        chrono::Utc::now(),
                        request.retention,
                    )
                    .map(Json)
                    .map_err(|_| StatusCode::BAD_REQUEST)
                },
            ),
        )
        .route("/agent/v1/jobs/{id}", get(read).put(submit))
        .route("/agent/v1/jobs/{id}/cancel", axum::routing::put(cancel))
        .route("/agent/v1/service/state", post(service))
        .route("/agent/v1/jobs/{id}/events", get(events))
        .with_state(control.clone());
    let agent = start_router(center, agent_id, app, control.0.epoch).await;
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
    control.0.cancels.fetch_add(1, Ordering::SeqCst);
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
        let restoring = control.0.restore_on_cancel.load(Ordering::SeqCst)
            && snapshot.job.as_ref().is_some_and(|job| {
                job.service_policy == hmail_cleaner_protocol::ServicePolicy::Manage
            });
        snapshot.phase = if restoring {
            JobPhase::Restoring
        } else {
            JobPhase::Cancelled
        };
        if restoring {
            snapshot.mail_state = ObservedMailState::Restoring;
        }
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
