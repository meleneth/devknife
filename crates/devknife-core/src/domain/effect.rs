use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Effect {
    Emit { event_type: String, payload: Value },
    Record { message: String },
    Assert(AssertEffect),
    Rest(RestEffect),
    Graphql(GraphqlEffect),
    SnsPublish(SnsPublishEffect),
    SqsSend(SqsSendEffect),
    SqsReceive(SqsReceiveEffect),
    Websocket(WebsocketEffect),
}

impl Effect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Emit { .. } => "emit",
            Self::Record { .. } => "record",
            Self::Assert(_) => "assert",
            Self::Rest(_) => "rest",
            Self::Graphql(_) => "graphql",
            Self::SnsPublish(_) => "sns_publish",
            Self::SqsSend(_) => "sqs_send",
            Self::SqsReceive(_) => "sqs_receive",
            Self::Websocket(_) => "websocket",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssertEffect {
    pub path: String,
    pub equals: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RestEffect {
    pub service: Option<String>,
    pub base_url: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    pub method: RestMethod,
    pub path: String,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub json_body: Option<Value>,
    #[serde(default)]
    pub expect: RestExpectations,
    #[serde(default)]
    pub emits: Vec<RestEventEmission>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RestMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl RestMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestExpectations {
    pub status: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestEventEmission {
    pub event_type: String,
    #[serde(default)]
    pub payload: BTreeMap<String, JsonPathSelector>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GraphqlEffect {
    pub service: Option<String>,
    pub base_url: Option<String>,
    pub operation_name: Option<String>,
    pub query: String,
    #[serde(default)]
    pub variables: Value,
    #[serde(default)]
    pub expect: GraphqlExpectations,
    #[serde(default)]
    pub emits: Vec<GraphqlEventEmission>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphqlExpectations {
    pub status: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphqlEventEmission {
    pub event_type: String,
    #[serde(default)]
    pub payload: BTreeMap<String, JsonPathSelector>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonPathSelector {
    pub from: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SnsPublishEffect {
    pub service: Option<String>,
    pub endpoint_url: Option<String>,
    pub topic_arn: String,
    pub message: Value,
    #[serde(default)]
    pub emits: Vec<SnsEventEmission>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnsEventEmission {
    pub event_type: String,
    #[serde(default)]
    pub payload: BTreeMap<String, JsonPathSelector>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SqsSendEffect {
    pub service: Option<String>,
    pub endpoint_url: Option<String>,
    pub queue_url: String,
    pub message: Value,
    #[serde(default)]
    pub emits: Vec<SqsEventEmission>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SqsReceiveEffect {
    pub service: Option<String>,
    pub endpoint_url: Option<String>,
    pub queue_url: String,
    #[serde(default = "default_sqs_max_messages")]
    pub max_messages: u8,
    #[serde(default)]
    pub wait_time_seconds: u8,
    #[serde(default)]
    pub delete_on_success: bool,
    #[serde(default)]
    pub emits: Vec<SqsEventEmission>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SqsEventEmission {
    pub event_type: String,
    #[serde(default)]
    pub payload: BTreeMap<String, JsonPathSelector>,
}

fn default_sqs_max_messages() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WebsocketEffect {
    pub service: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub session: Option<String>,
    pub send: WebsocketSend,
    #[serde(default)]
    pub expect: WebsocketExpectations,
    #[serde(default = "default_websocket_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub emits: Vec<WebsocketEventEmission>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebsocketSend {
    Json(Value),
    Text(String),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WebsocketExpectations {
    #[serde(default)]
    pub json: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WebsocketEventEmission {
    pub event_type: String,
    #[serde(default)]
    pub payload: BTreeMap<String, JsonPathSelector>,
}

fn default_websocket_timeout_ms() -> u64 {
    5_000
}
