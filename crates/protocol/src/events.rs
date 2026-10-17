use crate::{JobId, JobSnapshot, ProtocolError, ServerId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Counter(u64);
impl JsonSchema for Counter {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Counter".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "format": "uint64-decimal",
            "pattern": "^(0|[1-9][0-9]*)$",
            "maxLength": 20
        })
    }
}
impl Counter {
    pub fn get(self) -> u64 {
        self.0
    }
    pub fn next(self) -> Result<Self, ProtocolError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(ProtocolError::InvalidCounter)
    }
}
impl From<u64> for Counter {
    fn from(value: u64) -> Self {
        Self(value)
    }
}
impl Serialize for Counter {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for Counter {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let parsed = value
            .parse::<u64>()
            .map_err(|_| serde::de::Error::custom("invalid counter"))?;
        if parsed.to_string() != value {
            return Err(serde::de::Error::custom("noncanonical counter"));
        }
        Ok(Self(parsed))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SummaryDto {
    pub selected_bytes: Counter,
    pub selected_files: Counter,
    pub total_scan_files: Option<Counter>,
    pub verified_files: Counter,
    pub remaining_old_files: Option<Counter>,
    pub examined_messages: Counter,
    pub selected: Counter,
    pub removed_files: Counter,
    pub absent_files: Counter,
    pub deleted_messages: Counter,
    pub removed_directories: Counter,
    pub failed: Counter,
    pub removed_bytes: Counter,
    pub deleted_metadata: Counter,
    pub deleted_recipients: Counter,
    pub orphan_metadata: Counter,
    pub orphan_recipients: Counter,
    pub selected_orphan_metadata: Counter,
    pub selected_orphan_recipients: Counter,
    pub scanned_files: Counter,
    pub selected_orphan_files: Counter,
    pub removed_orphan_files: Counter,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub server_id: ServerId,
    pub job_id: Option<JobId>,
    pub stream_epoch: Uuid,
    pub revision: Counter,
    pub snapshot: JobSnapshot,
}
