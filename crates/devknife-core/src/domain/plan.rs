use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::{Effect, RestMethod, Workflow};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPlan {
    pub workflow_name: String,
    pub workflow_version: String,
    pub required_capabilities: Vec<Capability>,
    pub effects: Vec<PlannedEffect>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub risk: CapabilityRisk,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRisk {
    Read,
    Write,
    Local,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedEffect {
    pub handler_index: usize,
    pub effect_index: usize,
    pub on: String,
    pub effect_type: String,
    pub capabilities: Vec<String>,
}

pub fn plan_workflow(workflow: &Workflow) -> RunPlan {
    let mut required = BTreeSet::new();
    let mut effects = Vec::new();

    for (handler_index, handler) in workflow.handlers.iter().enumerate() {
        for (effect_index, effect) in handler.effects.iter().enumerate() {
            let capabilities = effect_capabilities(effect);
            for capability in &capabilities {
                required.insert(capability.clone());
            }
            effects.push(PlannedEffect {
                handler_index,
                effect_index,
                on: handler.on.clone(),
                effect_type: effect.name().to_string(),
                capabilities: capabilities
                    .into_iter()
                    .map(|capability| capability.id)
                    .collect(),
            });
        }
    }

    RunPlan {
        workflow_name: workflow.name.clone(),
        workflow_version: workflow.version.clone(),
        required_capabilities: required.into_iter().collect(),
        effects,
    }
}

fn effect_capabilities(effect: &Effect) -> Vec<Capability> {
    match effect {
        Effect::Emit { .. } => vec![capability(
            "workflow.emit",
            CapabilityRisk::Local,
            "Emit a new in-memory workflow event.",
        )],
        Effect::Record { .. } => vec![capability(
            "workflow.record",
            CapabilityRisk::Local,
            "Record a message in the run trace.",
        )],
        Effect::Assert(_) => vec![capability(
            "workflow.assert",
            CapabilityRisk::Local,
            "Evaluate an in-memory workflow assertion.",
        )],
        Effect::Rest(rest) => vec![capability(
            match rest.method {
                RestMethod::Get => "network.http.read",
                RestMethod::Post | RestMethod::Put | RestMethod::Patch | RestMethod::Delete => {
                    "network.http.write"
                }
            },
            match rest.method {
                RestMethod::Get => CapabilityRisk::Read,
                RestMethod::Post | RestMethod::Put | RestMethod::Patch | RestMethod::Delete => {
                    CapabilityRisk::Write
                }
            },
            "Call a REST HTTP endpoint.",
        )],
        Effect::Graphql(_) => vec![capability(
            "network.graphql",
            CapabilityRisk::Write,
            "Call a GraphQL endpoint.",
        )],
        Effect::SnsPublish(_) => vec![capability(
            "aws.sns.publish",
            CapabilityRisk::Write,
            "Publish a message to an SNS topic.",
        )],
        Effect::SqsSend(_) => vec![capability(
            "aws.sqs.send",
            CapabilityRisk::Write,
            "Send a message to an SQS queue.",
        )],
        Effect::SqsReceive(sqs) => {
            let mut capabilities = vec![capability(
                "aws.sqs.receive",
                CapabilityRisk::Read,
                "Receive messages from an SQS queue.",
            )];
            if sqs.delete_on_success {
                capabilities.push(capability(
                    "aws.sqs.delete",
                    CapabilityRisk::Write,
                    "Delete received SQS messages after successful processing.",
                ));
            }
            capabilities
        }
        Effect::Websocket(_) => vec![capability(
            "network.websocket",
            CapabilityRisk::Write,
            "Open a WebSocket connection and exchange messages.",
        )],
    }
}

fn capability(id: &str, risk: CapabilityRisk, description: &str) -> Capability {
    Capability {
        id: id.to_string(),
        risk,
        description: description.to_string(),
    }
}
