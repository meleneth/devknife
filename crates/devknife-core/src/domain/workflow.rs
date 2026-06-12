use serde::{Deserialize, Serialize};

use super::{Effect, Event};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workflow {
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
