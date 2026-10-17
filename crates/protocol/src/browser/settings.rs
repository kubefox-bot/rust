use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionSettings {
    pub idle_minutes: u16,
}
