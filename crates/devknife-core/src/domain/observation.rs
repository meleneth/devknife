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
    SnsPublish {
        operation: AwsOperationObservation,
        message_id: String,
        emitted_events: Vec<Event>,
    },
    SqsSend {
        operation: AwsOperationObservation,
        message_id: String,
        emitted_events: Vec<Event>,
    },
    SqsReceive {
        operation: AwsOperationObservation,
        messages: Vec<SqsMessageObservation>,
        deleted_receipt_handles: Vec<String>,
        emitted_events: Vec<Event>,
    },
    AwsFailed {
        operation: AwsOperationObservation,
        message: String,
    },
    WebsocketMessage {
        operation: WebsocketOperationObservation,
        sent: WebsocketSentObservation,
        received: WebsocketReceivedObservation,
        assertions: Vec<WebsocketAssertionObservation>,
        emitted_events: Vec<Event>,
    },
    WebsocketFailed {
        operation: WebsocketOperationObservation,
        message: String,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwsOperationObservation {
    pub service: Option<String>,
    pub action: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SqsMessageObservation {
    pub message_id: String,
    pub receipt_handle: String,
    pub body: Value,
    pub body_message_json: Option<Value>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebsocketOperationObservation {
    pub service: Option<String>,
    pub session: Option<String>,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebsocketSentObservation {
    Json { value: Value },
    Text { value: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebsocketReceivedObservation {
    Json { value: Value },
    Text { value: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebsocketAssertionObservation {
    JsonFieldPassed {
        path: String,
    },
    JsonFieldFailed {
        path: String,
        expected: Value,
        actual: Option<Value>,
    },
}
