use serde::{Deserialize, Serialize};

use super::{Effect, Event, Observation};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: String,
    pub workflow_name: String,
    pub status: RunStatus,
    pub trace: Vec<TraceEntry>,
    pub failure: Option<TraceFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceEntry {
    pub id: String,
    pub sequence: u64,
    pub kind: TraceEntryKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEntryKind {
    RunStarted {
        run_id: String,
        workflow_name: String,
    },
    EventSeeded {
        event: Event,
    },
    EventDequeued {
        event: Event,
    },
    HandlerMatched {
        event_id: String,
        handler_index: usize,
        on: String,
    },
    EffectExecuted {
        event_id: String,
        handler_index: usize,
        effect_index: usize,
        effect: Effect,
        observation: Observation,
    },
    HandlerSkipped {
        event_id: String,
        on: String,
    },
    RunEnded {
        status: RunStatus,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceFailure {
    pub trace_entry_id: Option<String>,
    pub message: String,
}
