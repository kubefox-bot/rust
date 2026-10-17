use super::{JobService, SubmitError};
use hmail_cleaner_protocol::{
    browser::jobs::{JobEvent, JobView},
    JobId,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::{broadcast, OwnedSemaphorePermit, Semaphore};
pub const EVENT_CAPACITY: usize = 256;
pub const MAX_FRAME_BYTES: usize = 4096;
const MAX_SUBSCRIBERS: usize = 16;
#[derive(Clone)]
pub struct EventHub(Arc<Hub>);
struct Hub {
    sender: broadcast::Sender<StreamItem>,
    revision: AtomicU64,
    slots: Arc<Semaphore>,
    epoch: uuid::Uuid,
}
#[derive(Debug, Clone)]
pub enum StreamItem {
    Snapshot(Box<JobEvent>),
    Resync,
}
pub struct Subscription {
    receiver: broadcast::Receiver<StreamItem>,
    _permit: OwnedSemaphorePermit,
}
impl Default for EventHub {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CAPACITY);
        Self(Arc::new(Hub {
            sender,
            revision: AtomicU64::new(0),
            slots: Arc::new(Semaphore::new(MAX_SUBSCRIBERS)),
            epoch: uuid::Uuid::new_v4(),
        }))
    }
}
impl EventHub {
    pub(super) fn epoch(&self) -> uuid::Uuid {
        self.0.epoch
    }
    pub fn subscribe(&self) -> Result<Subscription, SubmitError> {
        Ok(Subscription {
            receiver: self.0.sender.subscribe(),
            _permit: self
                .0
                .slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| SubmitError::Busy)?,
        })
    }
    pub fn publish(&self, job: JobView) {
        let Ok(prior) = self
            .0
            .revision
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
        else {
            let _ = self.0.sender.send(StreamItem::Resync);
            return;
        };
        let event = JobEvent {
            stream_epoch: self.0.epoch,
            revision: (prior + 1).into(),
            job,
        };
        let item = match serde_json::to_vec(&event) {
            Ok(bytes) if bytes.len() <= MAX_FRAME_BYTES => StreamItem::Snapshot(Box::new(event)),
            _ => StreamItem::Resync,
        };
        let _ = self.0.sender.send(item);
    }
}
impl Subscription {
    pub async fn recv(&mut self) -> Option<StreamItem> {
        match self.receiver.recv().await {
            Ok(item) => Some(item),
            Err(broadcast::error::RecvError::Lagged(_)) => Some(StreamItem::Resync),
            Err(broadcast::error::RecvError::Closed) => None,
        }
    }
}
impl JobService {
    pub(super) async fn publish(&self, id: JobId) -> Result<(), SubmitError> {
        self.events.publish(self.load(id).await?);
        Ok(())
    }
}
