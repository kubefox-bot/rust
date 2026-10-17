use super::{
    frames::{Frame, Frames},
    AgentClient, AgentError,
};
use bytes::Buf;
use hmail_cleaner_protocol::JobId;
use std::time::Duration;
const OPEN_TIMEOUT: Duration = Duration::from_secs(15);
const HEARTBEAT_DEADLINE: Duration = Duration::from_secs(45);
pub(crate) struct AgentStream<'a> {
    client: &'a AgentClient,
    response: reqwest::Response,
    chunk: bytes::Bytes,
    frames: Frames,
}
pub(crate) enum AgentEvent {
    Snapshot(Box<hmail_cleaner_protocol::JobSnapshot>),
    Resync,
}
impl AgentClient {
    pub(crate) async fn events(&self, id: JobId) -> Result<AgentStream<'_>, AgentError> {
        let mut revoked = self.lease.revocation();
        if !self.lease.is_live() {
            return Err(AgentError::Revoked);
        }
        let response = tokio::select! {biased;_=revoked.changed()=>return Err(AgentError::Revoked),_=self.trust.cancelled()=>return Err(AgentError::Revoked),result=tokio::time::timeout(OPEN_TIMEOUT,self.request(reqwest::Method::GET,&format!("/agent/v1/jobs/{id}/events")).send())=>result.map_err(|_|AgentError::Unavailable)?.map_err(|_|AgentError::Unavailable)?};
        if !response.status().is_success()
            || response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                != Some("text/event-stream")
        {
            return Err(AgentError::Protocol);
        }
        Ok(AgentStream {
            client: self,
            response,
            chunk: bytes::Bytes::new(),
            frames: Frames::default(),
        })
    }
}
impl AgentStream<'_> {
    pub(crate) async fn next(&mut self) -> Result<AgentEvent, AgentError> {
        let mut revoked = self.client.lease.revocation();
        if !self.client.lease.is_live() {
            return Err(AgentError::Revoked);
        }
        loop {
            while self.chunk.has_remaining() {
                if let Some(frame) = self.frames.push(self.chunk.get_u8())? {
                    return Ok(match frame {
                        Frame::Snapshot(snapshot) => AgentEvent::Snapshot(snapshot),
                        Frame::Resync => AgentEvent::Resync,
                    });
                }
            }
            self.chunk = tokio::select! {biased;_=revoked.changed()=>return Err(AgentError::Revoked),_=self.client.trust.cancelled()=>return Err(AgentError::Revoked),result=tokio::time::timeout(HEARTBEAT_DEADLINE,self.response.chunk())=>result.map_err(|_|AgentError::Unavailable)?.map_err(|_|AgentError::Unavailable)?.ok_or(AgentError::Unavailable)?};
        }
    }
}
