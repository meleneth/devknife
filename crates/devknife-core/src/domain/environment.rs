use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEnvironment {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub services: BTreeMap<String, ServiceBinding>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub secret_refs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceBinding {
    pub base_url: String,
}
