use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
        expected: Value,
        actual: Option<Value>,
    },
    RestResponse {
        operation: RestOperationObservation,
        response: RestResponseObservation,
        assertions: Vec<RestAssertionObservation>,
        emitted_events: Vec<Event>,
    },
    RestFailed {
        operation: RestOperationObservation,
        message: String,
        status: Option<u16>,
    },
    GraphqlResponse {
        operation: GraphqlOperationObservation,
        response: GraphqlResponseObservation,
        assertions: Vec<GraphqlAssertionObservation>,
        emitted_events: Vec<Event>,
    },
    GraphqlFailed {
        operation: GraphqlOperationObservation,
        message: String,
        status: Option<u16>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestOperationObservation {
    pub service: Option<String>,
    pub method: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestResponseObservation {
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: RestBody,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RestBody {
    Json { value: Value },
    Text { value: String },
    Empty,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RestAssertionObservation {
    StatusPassed { expected: u16, actual: u16 },
    StatusFailed { expected: u16, actual: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphqlOperationObservation {
    pub service: Option<String>,
    pub operation_name: Option<String>,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphqlResponseObservation {
    pub status: u16,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub data: Option<Value>,
    #[serde(default)]
    pub errors: Vec<Value>,
    pub extensions: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphqlAssertionObservation {
    StatusPassed { expected: u16, actual: u16 },
    StatusFailed { expected: u16, actual: u16 },
    NoErrorsPassed,
    NoErrorsFailed { errors: Vec<Value> },
}
