use super::errors::ApiError;
use crate::execution::{Executor, Worker};
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::{Stream, StreamExt};
use hmail_cleaner_protocol::JobId;
use std::{convert::Infallible, time::Duration};
use tokio_stream::wrappers::BroadcastStream;
const HEARTBEAT: Duration = Duration::from_secs(15);

pub(super) async fn get<E: Executor>(
    State(worker): State<Worker<E>>,
    Path(id): Path<JobId>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // Subscribe first so a terminal commit between subscription and GET is not lost.
    let receiver = worker.subscribe();
    let initial = worker.lookup(id).await?.ok_or_else(ApiError::not_found)?;
    let stream = BroadcastStream::new(receiver).filter_map(move |event| {
        futures_util::future::ready(match event {
            Ok(snapshot) if snapshot.job_id == id => Some(
                Event::default()
                    .event("snapshot")
                    .json_data(snapshot)
                    .expect("public snapshot serializes"),
            ),
            Ok(_) => None,
            Err(_) => Some(Event::default().event("resync").data("{}")),
        })
    });
    let initial = futures_util::stream::once(async move {
        Event::default()
            .event("snapshot")
            .json_data(initial)
            .expect("public snapshot serializes")
    });
    Ok(Sse::new(initial.chain(stream).map(Ok)).keep_alive(KeepAlive::new().interval(HEARTBEAT)))
}
