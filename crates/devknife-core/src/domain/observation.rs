use serde::{Deserialize, Serialize};

use super::Event;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Observation {
    EmittedEvents {
        events: Vec<Event>,
    },
    RecordedMessage {
        message: String,
    },
    AssertionPassed {
        path: String,
    },
    AssertionFailed {
        path: String,
        expected: serde_json::Value,
        actual: Option<serde_json::Value>,
    },
}
