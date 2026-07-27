use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use thiserror::Error;

use crate::domain::{
    default_workflow_version, Effect, Event, Handler, JsonPathSelector, RuntimeEnvironment,
    Workflow, CURRENT_WORKFLOW_VERSION,
};

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

pub fn load_environment_yaml(input: &str) -> Result<RuntimeEnvironment, LoadError> {
    let environment: RuntimeEnvironment = serde_yml::from_str(input)?;
    validate_environment(&environment)?;
    Ok(environment)
}

pub fn validate_environment(environment: &RuntimeEnvironment) -> Result<(), LoadError> {
    for (service, binding) in &environment.services {
        if service.trim().is_empty() {
            return Err(LoadError::Validation(
                "environment service name is required".to_string(),
            ));
        }
        if binding.base_url.trim().is_empty() {
            return Err(LoadError::Validation(format!(
                "environment service {service}.base_url is required"
            )));
        }
    }

    for (name, value) in &environment.secret_refs {
        if name.trim().is_empty() {
            return Err(LoadError::Validation(
                "environment secret reference name is required".to_string(),
            ));
        }
        if value.is_empty() {
            return Err(LoadError::Validation(format!(
                "environment secret reference '{name}' must not be empty"
            )));
        }
    }

    Ok(())
}

pub fn validate_workflow_environment(
    workflow: &Workflow,
    environment: &RuntimeEnvironment,
) -> Result<(), LoadError> {
    let mut missing = BTreeSet::new();

    for handler in &workflow.handlers {
        for effect in &handler.effects {
            let service = match effect {
                Effect::Rest(effect) => effect.service.as_deref(),
                Effect::Graphql(effect) => effect.service.as_deref(),
                Effect::SnsPublish(effect) => effect.service.as_deref(),
                Effect::SqsSend(effect) => effect.service.as_deref(),
                Effect::SqsReceive(effect) => effect.service.as_deref(),
                Effect::Websocket(effect) => effect.service.as_deref(),
                Effect::Emit { .. } | Effect::Record { .. } | Effect::Assert(_) => None,
            };
            if let Some(service) = service {
                if !environment.services.contains_key(service) {
                    missing.insert(format!("service '{service}'"));
                }
            }
        }
    }

    let document = serde_json::to_value(workflow).expect("workflow serializes");
    collect_missing_template_bindings(&document, environment, &mut missing);

    if missing.is_empty() {
        Ok(())
    } else {
        Err(LoadError::Validation(format!(
            "missing environment bindings: {}",
            missing.into_iter().collect::<Vec<_>>().join(", ")
        )))
    }
}

fn collect_missing_template_bindings(
    value: &Value,
    environment: &RuntimeEnvironment,
    missing: &mut BTreeSet<String>,
) {
    match value {
        Value::String(value) => {
            let mut remaining = value.as_str();
            while let Some(start) = remaining.find("{{") {
                let after_start = &remaining[start + 2..];
                let Some(end) = after_start.find("}}") else {
                    break;
                };
                let expression = after_start[..end].trim();
                if let Some(name) = expression.strip_prefix("env.") {
                    if !environment.values.contains_key(name) {
                        missing.insert(format!("value '{name}'"));
                    }
                } else if let Some(name) = expression.strip_prefix("secret.") {
                    if !environment.secret_refs.contains_key(name) {
                        missing.insert(format!("secret '{name}'"));
                    }
                }
                remaining = &after_start[end + 2..];
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_missing_template_bindings(value, environment, missing);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_missing_template_bindings(value, environment, missing);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

pub fn validate_workflow(workflow: &Workflow) -> Result<(), LoadError> {
    if workflow.version.trim().is_empty() {
        return Err(LoadError::Validation(
            "workflow version is required".to_string(),
        ));
    }
    if workflow.version != CURRENT_WORKFLOW_VERSION {
        return Err(LoadError::Validation(format!(
            "unsupported workflow version '{}'; expected {CURRENT_WORKFLOW_VERSION}",
            workflow.version
        )));
    }
    if workflow.name.trim().is_empty() {
        return Err(LoadError::Validation(
            "workflow name is required".to_string(),
        ));
    }

    let document = serde_json::to_value(workflow).expect("workflow serializes");
    validate_template_expressions(&document)?;

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

fn validate_template_expressions(value: &Value) -> Result<(), LoadError> {
    match value {
        Value::String(value) => validate_template_string(value),
        Value::Array(values) => {
            for value in values {
                validate_template_expressions(value)?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for value in values.values() {
                validate_template_expressions(value)?;
            }
            Ok(())
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
    }
}

fn validate_template_string(value: &str) -> Result<(), LoadError> {
    let mut remaining = value;
    loop {
        let opening = remaining.find("{{");
        let closing = remaining.find("}}");
        if closing.is_some_and(|closing| opening.is_none_or(|opening| closing < opening)) {
            return Err(LoadError::Validation(format!(
                "unexpected template closing delimiter in '{value}'"
            )));
        }

        let Some(opening) = opening else {
            return Ok(());
        };
        let after_opening = &remaining[opening + 2..];
        let Some(closing) = after_opening.find("}}") else {
            return Err(LoadError::Validation(format!(
                "unclosed template expression in '{value}'"
            )));
        };
        let expression = after_opening[..closing].trim();
        let valid = ["event.payload.", "env.", "secret."].iter().any(|prefix| {
            expression
                .strip_prefix(prefix)
                .is_some_and(|name| !name.is_empty())
        });
        if !valid {
            return Err(LoadError::Validation(format!(
                "unsupported template expression '{expression}'"
            )));
        }
        remaining = &after_opening[closing + 2..];
    }
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
        Effect::Rest(rest) => {
            if rest
                .service
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
                && rest
                    .base_url
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
            {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}] requires service or base_url"
                )));
            }
            if rest.path.trim().is_empty() {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}].path is required"
                )));
            }
            for (emit_index, emit) in rest.emits.iter().enumerate() {
                if emit.event_type.trim().is_empty() {
                    return Err(LoadError::Validation(format!(
                        "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].event_type is required"
                    )));
                }
                validate_json_path_payload(&emit.payload, handler_index, effect_index, emit_index)?;
            }
            Ok(())
        }
        Effect::Graphql(graphql) => {
            if graphql
                .service
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
                && graphql
                    .base_url
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
            {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}] requires service or base_url"
                )));
            }
            if graphql.query.trim().is_empty() {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}].query is required"
                )));
            }
            for (emit_index, emit) in graphql.emits.iter().enumerate() {
                if emit.event_type.trim().is_empty() {
                    return Err(LoadError::Validation(format!(
                        "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].event_type is required"
                    )));
                }
                validate_json_path_payload(&emit.payload, handler_index, effect_index, emit_index)?;
            }
            Ok(())
        }
        Effect::SnsPublish(sns) => {
            validate_aws_binding(
                sns.service.as_deref(),
                sns.endpoint_url.as_deref(),
                handler_index,
                effect_index,
            )?;
            if sns.topic_arn.trim().is_empty() {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}].topic_arn is required"
                )));
            }
            for (emit_index, emit) in sns.emits.iter().enumerate() {
                if emit.event_type.trim().is_empty() {
                    return Err(LoadError::Validation(format!(
                        "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].event_type is required"
                    )));
                }
                validate_json_path_payload(&emit.payload, handler_index, effect_index, emit_index)?;
            }
            Ok(())
        }
        Effect::SqsSend(sqs) => {
            validate_aws_binding(
                sqs.service.as_deref(),
                sqs.endpoint_url.as_deref(),
                handler_index,
                effect_index,
            )?;
            if sqs.queue_url.trim().is_empty() {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}].queue_url is required"
                )));
            }
            for (emit_index, emit) in sqs.emits.iter().enumerate() {
                if emit.event_type.trim().is_empty() {
                    return Err(LoadError::Validation(format!(
                        "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].event_type is required"
                    )));
                }
                validate_json_path_payload(&emit.payload, handler_index, effect_index, emit_index)?;
            }
            Ok(())
        }
        Effect::SqsReceive(sqs) => {
            validate_aws_binding(
                sqs.service.as_deref(),
                sqs.endpoint_url.as_deref(),
                handler_index,
                effect_index,
            )?;
            if sqs.queue_url.trim().is_empty() {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}].queue_url is required"
                )));
            }
            if sqs.max_messages == 0 || sqs.max_messages > 10 {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}].max_messages must be between 1 and 10"
                )));
            }
            for (emit_index, emit) in sqs.emits.iter().enumerate() {
                if emit.event_type.trim().is_empty() {
                    return Err(LoadError::Validation(format!(
                        "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].event_type is required"
                    )));
                }
                validate_json_path_payload(&emit.payload, handler_index, effect_index, emit_index)?;
            }
            Ok(())
        }
        Effect::Websocket(websocket) => {
            if websocket
                .service
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
                && websocket
                    .url
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
            {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}] requires service or url"
                )));
            }
            if websocket.timeout_ms == 0 {
                return Err(LoadError::Validation(format!(
                    "handlers[{handler_index}].effects[{effect_index}].timeout_ms must be greater than 0"
                )));
            }
            for path in websocket.expect.json.keys() {
                jsonpath_rfc9535::JsonPath::parse(path).map_err(|error| {
                    LoadError::Validation(format!(
                        "handlers[{handler_index}].effects[{effect_index}].expect.json.{path} is not valid JSONPath: {error}"
                    ))
                })?;
            }
            for (emit_index, emit) in websocket.emits.iter().enumerate() {
                if emit.event_type.trim().is_empty() {
                    return Err(LoadError::Validation(format!(
                        "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].event_type is required"
                    )));
                }
                validate_json_path_payload(&emit.payload, handler_index, effect_index, emit_index)?;
            }
            Ok(())
        }
        Effect::Emit { .. } | Effect::Record { .. } | Effect::Assert(_) => Ok(()),
    }
}

fn validate_aws_binding(
    service: Option<&str>,
    endpoint_url: Option<&str>,
    handler_index: usize,
    effect_index: usize,
) -> Result<(), LoadError> {
    if service.unwrap_or_default().trim().is_empty()
        && endpoint_url.unwrap_or_default().trim().is_empty()
    {
        return Err(LoadError::Validation(format!(
            "handlers[{handler_index}].effects[{effect_index}] requires service or endpoint_url"
        )));
    }

    Ok(())
}

fn validate_json_path_payload(
    payload: &std::collections::BTreeMap<String, JsonPathSelector>,
    handler_index: usize,
    effect_index: usize,
    emit_index: usize,
) -> Result<(), LoadError> {
    for (field, selector) in payload {
        if field.trim().is_empty() {
            return Err(LoadError::Validation(format!(
                "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].payload field name is required"
            )));
        }
        if selector.from.trim().is_empty() {
            return Err(LoadError::Validation(format!(
                "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].payload.{field}.from is required"
            )));
        }
        jsonpath_rfc9535::JsonPath::parse(&selector.from).map_err(|error| {
            LoadError::Validation(format!(
                "handlers[{handler_index}].effects[{effect_index}].emits[{emit_index}].payload.{field}.from is not valid JSONPath: {error}"
            ))
        })?;
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowDocument {
    #[serde(default = "default_workflow_version")]
    version: String,
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
            version: self.version,
            name: self.name,
            seed_events,
            handlers: self.handlers,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedEventDocument {
    id: Option<String>,
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    payload: Value,
}
