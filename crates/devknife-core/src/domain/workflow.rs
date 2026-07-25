use serde::{Deserialize, Serialize};

use super::{Effect, Event};

pub const CURRENT_WORKFLOW_VERSION: &str = "devknife.workflow/v1alpha1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
    #[serde(default = "default_workflow_version")]
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub seed_events: Vec<Event>,
    #[serde(default)]
    pub handlers: Vec<Handler>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Handler {
    pub on: String,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

pub fn default_workflow_version() -> String {
    CURRENT_WORKFLOW_VERSION.to_string()
}
