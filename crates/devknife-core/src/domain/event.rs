use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: String,
    #[serde(default)]
    pub payload: Value,
    pub caused_by: Option<EventCause>,
    pub sequence: u64,
    pub depth: u32,
}

impl Event {
    pub fn seed(id: impl Into<String>, event_type: impl Into<String>, payload: Value) -> Self {
        Self {
            id: id.into(),
            event_type: event_type.into(),
            payload,
            caused_by: None,
            sequence: 0,
            depth: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCause {
    pub event_id: String,
    pub trace_entry_id: String,
}
