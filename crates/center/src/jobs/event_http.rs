use super::StreamItem;
use crate::{
    auth::http::lease,
    http::{errors::ApiError, AppState},
};
use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use hmail_cleaner_protocol::ServerId;
use serde::Deserialize;
use tower_cookies::Cookies;
#[derive(Deserialize)]
pub(super) struct Scope {
    server_id: ServerId,
}
pub(super) async fn events(
    State(state): State<AppState>,
    cookies: Cookies,
    Query(scope): Query<Scope>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>>, ApiError>
{
    let current = lease(&state, &cookies)?;
    let subscription = state.jobs.events().subscribe()?;
    let stream = futures_util::stream::unfold(
        (subscription, current, true),
        move |(mut subscription, current, initial)| async move {
            if !current.is_live() {
                return None;
            }
            if initial {
                return Some((
                    Ok(Event::default().event("resync").data("{}")),
                    (subscription, current, false),
                ));
            }
            let mut revoked = current.revocation();
            loop {
                let item = tokio::select! {biased;_=revoked.changed()=>return None,item=subscription.recv()=>item?};
                let event = match item {
                    StreamItem::Snapshot(event)
                        if event.job.intent.server_id == scope.server_id =>
                    {
                        Event::default().event("snapshot").json_data(*event).ok()?
                    }
                    StreamItem::Snapshot(_) => continue,
                    StreamItem::Resync => Event::default().event("resync").data("{}"),
                };
                return Some((Ok(event), (subscription, current, false)));
            }
        },
    );
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15))))
}
