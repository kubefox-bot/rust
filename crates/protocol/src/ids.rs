use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

macro_rules! identifier {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);
        impl JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> { stringify!($name).into() }
            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({"type":"string", "format":"canonical-uuid"})
            }
        }
        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
            pub fn uuid(self) -> Uuid {
                self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let input = String::deserialize(deserializer)?;
                let parsed = Uuid::parse_str(&input)
                    .map_err(|_| serde::de::Error::custom("invalid identifier"))?;
                if parsed.is_nil() || input != parsed.hyphenated().to_string() {
                    return Err(serde::de::Error::custom(
                        "identifier must be a nonzero canonical UUID",
                    ));
                }
                Ok(Self(parsed))
            }
        }
    };
}
identifier!(JobId);
identifier!(AgentId);
identifier!(ProfileId);
identifier!(ServerId);
