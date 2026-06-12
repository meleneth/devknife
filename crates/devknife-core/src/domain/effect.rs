use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Effect {
    Emit { event_type: String, payload: Value },
    Record { message: String },
    Assert(AssertEffect),
}

impl Effect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Emit { .. } => "emit",
            Self::Record { .. } => "record",
            Self::Assert(_) => "assert",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssertEffect {
    pub path: String,
    pub equals: Value,
}
