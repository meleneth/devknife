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
}

impl Effect {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Emit { .. } => "emit",
            Self::Record { .. } => "record",
            Self::Assert(_) => "assert",
            Self::Rest(_) => "rest",
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
    pub payload: BTreeMap<String, ResponsePath>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponsePath {
    Path(String),
    From { from: String },
}

impl ResponsePath {
    pub fn as_path(&self) -> &str {
        match self {
            Self::Path(path) => path,
            Self::From { from } => from,
        }
    }
}
