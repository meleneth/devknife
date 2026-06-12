use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::domain::{Effect, Event, Handler, Workflow};

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("failed to parse workflow YAML: {0}")]
    Parse(#[from] serde_yml::Error),
    #[error("workflow validation failed: {0}")]
    Validation(String),
}

pub fn load_workflow_yaml(input: &str) -> Result<Workflow, LoadError> {
    let document: WorkflowDocument = serde_yml::from_str(input)?;
    let workflow = document.into_workflow();
    validate_workflow(&workflow)?;
    Ok(workflow)
}

pub fn validate_workflow(workflow: &Workflow) -> Result<(), LoadError> {
    if workflow.name.trim().is_empty() {
        return Err(LoadError::Validation(
            "workflow name is required".to_string(),
        ));
    }

    for (index, event) in workflow.seed_events.iter().enumerate() {
        if event.event_type.trim().is_empty() {
            return Err(LoadError::Validation(format!(
                "seed_events[{index}].type is required"
            )));
        }
    }

    for (handler_index, handler) in workflow.handlers.iter().enumerate() {
        if handler.on.trim().is_empty() {
            return Err(LoadError::Validation(format!(
                "handlers[{handler_index}].on is required"
            )));
        }

        for (effect_index, effect) in handler.effects.iter().enumerate() {
            validate_effect(effect, handler_index, effect_index)?;
        }
    }

    Ok(())
}

fn validate_effect(
    effect: &Effect,
    handler_index: usize,
    effect_index: usize,
) -> Result<(), LoadError> {
    match effect {
        Effect::Emit { event_type, .. } if event_type.trim().is_empty() => {
            Err(LoadError::Validation(format!(
                "handlers[{handler_index}].effects[{effect_index}].event_type is required"
            )))
        }
        Effect::Assert(assertion) if assertion.path.trim().is_empty() => {
            Err(LoadError::Validation(format!(
                "handlers[{handler_index}].effects[{effect_index}].path is required"
            )))
        }
        Effect::Emit { .. } | Effect::Record { .. } | Effect::Assert(_) => Ok(()),
    }
}

#[derive(Debug, Deserialize)]
struct WorkflowDocument {
    name: String,
    #[serde(default)]
    seed_events: Vec<SeedEventDocument>,
    #[serde(default)]
    handlers: Vec<Handler>,
}

impl WorkflowDocument {
    fn into_workflow(self) -> Workflow {
        let seed_events = self
            .seed_events
            .into_iter()
            .enumerate()
            .map(|(index, event)| {
                Event::seed(
                    event.id.unwrap_or_else(|| format!("seed-{}", index + 1)),
                    event.event_type,
                    event.payload,
                )
            })
            .collect();

        Workflow {
            name: self.name,
            seed_events,
            handlers: self.handlers,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SeedEventDocument {
    id: Option<String>,
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    payload: Value,
}
