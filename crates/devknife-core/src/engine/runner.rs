use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use serde_json::Value;
use uuid::Uuid;

use crate::domain::{
    AssertEffect, Effect, Event, EventCause, GraphqlAssertionObservation, GraphqlEffect,
    GraphqlOperationObservation, GraphqlResponseObservation, JsonPathSelector, Observation,
    RestAssertionObservation, RestBody, RestEffect, RestOperationObservation,
    RestResponseObservation, RunReport, RunStatus, RuntimeEnvironment, TraceEntry, TraceEntryKind,
    TraceFailure, Workflow,
};

use super::EngineError;

#[derive(Clone, Debug)]
pub struct ExecutionLimits {
    pub max_events: usize,
    pub max_steps: usize,
    pub max_depth: u32,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_events: 1_000,
            max_steps: 2_000,
            max_depth: 100,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Runner {
    limits: ExecutionLimits,
    environment: RuntimeEnvironment,
}

impl Runner {
    pub fn new(limits: ExecutionLimits) -> Self {
        Self {
            limits,
            environment: RuntimeEnvironment::default(),
        }
    }

    pub fn with_environment(limits: ExecutionLimits, environment: RuntimeEnvironment) -> Self {
        Self {
            limits,
            environment,
        }
    }

    pub fn run(&self, workflow: Workflow) -> RunReport {
        let run_id = Uuid::new_v4().to_string();
        let mut state = RunState::new(run_id, workflow.name.clone());

        state.push_trace(TraceEntryKind::RunStarted {
            run_id: state.run_id.clone(),
            workflow_name: workflow.name.clone(),
        });

        let mut queue = VecDeque::new();
        let mut created_events = 0usize;

        for mut event in workflow.seed_events {
            created_events += 1;
            event.sequence = created_events as u64;
            state.push_trace(TraceEntryKind::EventSeeded {
                event: event.clone(),
            });
            queue.push_back(event);
        }

        let mut processed_steps = 0usize;

        while let Some(event) = queue.pop_front() {
            if let Err(error) = self.check_step_limit(processed_steps) {
                return state.fail(None, error.to_string());
            }
            processed_steps += 1;

            state.push_trace(TraceEntryKind::EventDequeued {
                event: event.clone(),
            });

            let mut matched_any = false;
            for (handler_index, handler) in workflow.handlers.iter().enumerate() {
                if handler.on != event.event_type {
                    continue;
                }

                matched_any = true;
                state.push_trace(TraceEntryKind::HandlerMatched {
                    event_id: event.id.clone(),
                    handler_index,
                    on: handler.on.clone(),
                });

                for (effect_index, effect) in handler.effects.iter().enumerate() {
                    if let Err(error) = self.check_step_limit(processed_steps) {
                        return state.fail(None, error.to_string());
                    }
                    processed_steps += 1;

                    let trace_entry_id = state.next_trace_id();
                    let observation = match self.execute_effect(
                        effect,
                        &event,
                        &trace_entry_id,
                        &mut created_events,
                    ) {
                        Ok(observation) => observation,
                        Err(error) => return state.fail(Some(trace_entry_id), error.to_string()),
                    };

                    if observation_failed(&observation) {
                        let failure_message = format!(
                            "event {} triggered handler {} effect {}: {}",
                            event.id,
                            handler_index,
                            effect_index,
                            observation_failure_message(&observation)
                        );
                        state.push_trace_with_id(
                            trace_entry_id.clone(),
                            TraceEntryKind::EffectExecuted {
                                event_id: event.id.clone(),
                                handler_index,
                                effect_index,
                                effect: effect.clone(),
                                observation,
                            },
                        );
                        return state.fail(Some(trace_entry_id), failure_message);
                    }

                    let emitted = match &observation {
                        Observation::EmittedEvents { events } => events.clone(),
                        Observation::RestResponse { emitted_events, .. } => emitted_events.clone(),
                        Observation::GraphqlResponse { emitted_events, .. } => {
                            emitted_events.clone()
                        }
                        _ => Vec::new(),
                    };

                    state.push_trace_with_id(
                        trace_entry_id,
                        TraceEntryKind::EffectExecuted {
                            event_id: event.id.clone(),
                            handler_index,
                            effect_index,
                            effect: effect.clone(),
                            observation,
                        },
                    );

                    for emitted_event in emitted {
                        queue.push_back(emitted_event);
                    }
                }
            }

            if !matched_any {
                state.push_trace(TraceEntryKind::HandlerSkipped {
                    event_id: event.id,
                    on: event.event_type,
                });
            }
        }

        state.succeed()
    }

    fn execute_effect(
        &self,
        effect: &Effect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
    ) -> Result<Observation, EngineError> {
        match effect {
            Effect::Emit {
                event_type,
                payload,
            } => {
                if *created_events >= self.limits.max_events {
                    return Err(EngineError::MaxEventCountExceeded {
                        limit: self.limits.max_events,
                    });
                }

                let next_depth = event.depth + 1;
                if next_depth > self.limits.max_depth {
                    return Err(EngineError::MaxDepthExceeded {
                        limit: self.limits.max_depth,
                    });
                }

                *created_events += 1;
                let emitted = Event {
                    id: format!("event-{}", created_events),
                    event_type: event_type.clone(),
                    payload: payload.clone(),
                    caused_by: Some(EventCause {
                        event_id: event.id.clone(),
                        trace_entry_id: trace_entry_id.to_string(),
                    }),
                    sequence: *created_events as u64,
                    depth: next_depth,
                };

                Ok(Observation::EmittedEvents {
                    events: vec![emitted],
                })
            }
            Effect::Record { message } => Ok(Observation::RecordedMessage {
                message: message.clone(),
            }),
            Effect::Assert(assertion) => Ok(evaluate_assertion(assertion, &event.payload)),
            Effect::Rest(rest) => {
                self.execute_rest_effect(rest, event, trace_entry_id, created_events)
            }
            Effect::Graphql(graphql) => {
                self.execute_graphql_effect(graphql, event, trace_entry_id, created_events)
            }
        }
    }

    fn execute_rest_effect(
        &self,
        rest: &RestEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
    ) -> Result<Observation, EngineError> {
        let request = match build_rest_request(rest, event, &self.environment) {
            Ok(request) => request,
            Err(error) => {
                return Ok(Observation::RestFailed {
                    operation: fallback_rest_operation(rest, event, &self.environment),
                    message: error.to_string(),
                    status: None,
                });
            }
        };
        let response = match execute_http_request(&request) {
            Ok(response) => response,
            Err(error) => {
                return Ok(Observation::RestFailed {
                    operation: RestOperationObservation {
                        service: rest.service.clone(),
                        method: rest.method.as_str().to_string(),
                        url: request.url,
                    },
                    message: error.to_string(),
                    status: None,
                });
            }
        };
        let assertions = evaluate_rest_assertions(rest, response.status);
        let emitted_events =
            self.build_rest_emitted_events(rest, event, trace_entry_id, created_events, &response)?;

        Ok(Observation::RestResponse {
            operation: RestOperationObservation {
                service: rest.service.clone(),
                method: rest.method.as_str().to_string(),
                url: request.url,
            },
            response,
            assertions,
            emitted_events,
        })
    }

    fn build_rest_emitted_events(
        &self,
        rest: &RestEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
        response: &RestResponseObservation,
    ) -> Result<Vec<Event>, EngineError> {
        let mut emitted_events = Vec::new();
        let extraction_document = rest_extraction_document(response);

        for emission in &rest.emits {
            if *created_events >= self.limits.max_events {
                return Err(EngineError::MaxEventCountExceeded {
                    limit: self.limits.max_events,
                });
            }

            let next_depth = event.depth + 1;
            if next_depth > self.limits.max_depth {
                return Err(EngineError::MaxDepthExceeded {
                    limit: self.limits.max_depth,
                });
            }

            let mut payload = serde_json::Map::new();
            for (field, path) in &emission.payload {
                let value =
                    select_json_path_value(&extraction_document, path).map_err(|error| {
                        EngineError::RestEmissionPathInvalid {
                            event_type: emission.event_type.clone(),
                            path: path.from.clone(),
                            message: error,
                        }
                    })?;
                let value = value.ok_or_else(|| EngineError::RestEmissionPathMissing {
                    event_type: emission.event_type.clone(),
                    path: path.from.clone(),
                })?;
                payload.insert(field.clone(), value);
            }

            *created_events += 1;
            emitted_events.push(Event {
                id: format!("event-{}", created_events),
                event_type: emission.event_type.clone(),
                payload: Value::Object(payload),
                caused_by: Some(EventCause {
                    event_id: event.id.clone(),
                    trace_entry_id: trace_entry_id.to_string(),
                }),
                sequence: *created_events as u64,
                depth: next_depth,
            });
        }

        Ok(emitted_events)
    }

    fn execute_graphql_effect(
        &self,
        graphql: &GraphqlEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
    ) -> Result<Observation, EngineError> {
        let request = match build_graphql_request(graphql, event, &self.environment) {
            Ok(request) => request,
            Err(error) => {
                return Ok(Observation::GraphqlFailed {
                    operation: fallback_graphql_operation(graphql, event, &self.environment),
                    message: error.to_string(),
                    status: None,
                });
            }
        };
        let raw_response = match execute_http_request(&request) {
            Ok(response) => response,
            Err(error) => {
                return Ok(Observation::GraphqlFailed {
                    operation: GraphqlOperationObservation {
                        service: graphql.service.clone(),
                        operation_name: graphql.operation_name.clone(),
                        url: request.url,
                    },
                    message: error.to_string(),
                    status: None,
                });
            }
        };
        let status = raw_response.status;
        let response = match graphql_response_from_http(&request.operation, raw_response) {
            Ok(response) => response,
            Err(error) => {
                return Ok(Observation::GraphqlFailed {
                    operation: GraphqlOperationObservation {
                        service: graphql.service.clone(),
                        operation_name: graphql.operation_name.clone(),
                        url: request.url,
                    },
                    message: error.to_string(),
                    status: Some(status),
                });
            }
        };
        let assertions = evaluate_graphql_assertions(graphql, &response);
        let emitted_events = self.build_graphql_emitted_events(
            graphql,
            event,
            trace_entry_id,
            created_events,
            &response,
        )?;

        Ok(Observation::GraphqlResponse {
            operation: GraphqlOperationObservation {
                service: graphql.service.clone(),
                operation_name: graphql.operation_name.clone(),
                url: request.url,
            },
            response,
            assertions,
            emitted_events,
        })
    }

    fn build_graphql_emitted_events(
        &self,
        graphql: &GraphqlEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
        response: &GraphqlResponseObservation,
    ) -> Result<Vec<Event>, EngineError> {
        let mut emitted_events = Vec::new();
        let extraction_document = graphql_extraction_document(response);

        for emission in &graphql.emits {
            if *created_events >= self.limits.max_events {
                return Err(EngineError::MaxEventCountExceeded {
                    limit: self.limits.max_events,
                });
            }

            let next_depth = event.depth + 1;
            if next_depth > self.limits.max_depth {
                return Err(EngineError::MaxDepthExceeded {
                    limit: self.limits.max_depth,
                });
            }

            let mut payload = serde_json::Map::new();
            for (field, path) in &emission.payload {
                let value =
                    select_json_path_value(&extraction_document, path).map_err(|error| {
                        EngineError::GraphqlEmissionPathInvalid {
                            event_type: emission.event_type.clone(),
                            path: path.from.clone(),
                            message: error,
                        }
                    })?;
                let value = value.ok_or_else(|| EngineError::GraphqlEmissionPathMissing {
                    event_type: emission.event_type.clone(),
                    path: path.from.clone(),
                })?;
                payload.insert(field.clone(), value);
            }

            *created_events += 1;
            emitted_events.push(Event {
                id: format!("event-{}", created_events),
                event_type: emission.event_type.clone(),
                payload: Value::Object(payload),
                caused_by: Some(EventCause {
                    event_id: event.id.clone(),
                    trace_entry_id: trace_entry_id.to_string(),
                }),
                sequence: *created_events as u64,
                depth: next_depth,
            });
        }

        Ok(emitted_events)
    }

    fn check_step_limit(&self, processed_steps: usize) -> Result<(), EngineError> {
        if processed_steps >= self.limits.max_steps {
            Err(EngineError::MaxStepCountExceeded {
                limit: self.limits.max_steps,
            })
        } else {
            Ok(())
        }
    }
}

impl Default for Runner {
    fn default() -> Self {
        Self::new(ExecutionLimits::default())
    }
}

fn evaluate_assertion(assertion: &AssertEffect, payload: &Value) -> Observation {
    let actual = lookup_payload_path(payload, &assertion.path).cloned();
    if actual.as_ref() == Some(&assertion.equals) {
        Observation::AssertionPassed {
            path: assertion.path.clone(),
        }
    } else {
        Observation::AssertionFailed {
            path: assertion.path.clone(),
            expected: assertion.equals.clone(),
            actual,
        }
    }
}

fn observation_failed(observation: &Observation) -> bool {
    match observation {
        Observation::AssertionFailed { .. } => true,
        Observation::RestResponse { assertions, .. } => assertions
            .iter()
            .any(|assertion| matches!(assertion, RestAssertionObservation::StatusFailed { .. })),
        Observation::RestFailed { .. } => true,
        Observation::GraphqlResponse { assertions, .. } => assertions.iter().any(|assertion| {
            matches!(
                assertion,
                GraphqlAssertionObservation::StatusFailed { .. }
                    | GraphqlAssertionObservation::NoErrorsFailed { .. }
            )
        }),
        Observation::GraphqlFailed { .. } => true,
        _ => false,
    }
}

fn observation_failure_message(observation: &Observation) -> String {
    match observation {
        Observation::AssertionFailed {
            path,
            expected,
            actual,
        } => format!(
            "assertion failed during workflow run: path {path} expected {expected}, actual {}",
            actual
                .as_ref()
                .map(Value::to_string)
                .unwrap_or_else(|| "<missing>".to_string())
        ),
        Observation::RestResponse {
            operation,
            response,
            assertions,
            ..
        } => assertions
            .iter()
            .find_map(|assertion| match assertion {
                RestAssertionObservation::StatusFailed { expected, actual } => Some(format!(
                    "REST assertion failed during workflow run: event-triggered handler executed {} {}; status expected {}, actual {}",
                    operation.method, operation.url, expected, actual
                )),
                RestAssertionObservation::StatusPassed { .. } => None,
            })
            .unwrap_or_else(|| {
                format!(
                    "REST effect failed during workflow run: {} {} returned status {}",
                    operation.method, operation.url, response.status
                )
            }),
        Observation::RestFailed {
            operation,
            message,
            status,
        } => format!(
            "REST effect failed during workflow run: {} {}{}: {}",
            operation.method,
            operation.url,
            status
                .map(|status| format!(" returned status {status}"))
                .unwrap_or_default(),
            message
        ),
        Observation::GraphqlResponse {
            operation,
            response,
            assertions,
            ..
        } => assertions
            .iter()
            .find_map(|assertion| match assertion {
                GraphqlAssertionObservation::StatusFailed { expected, actual } => Some(format!(
                    "GraphQL assertion failed during workflow run: {} at {}; status expected {}, actual {}",
                    operation
                        .operation_name
                        .as_deref()
                        .unwrap_or("anonymous operation"),
                    operation.url,
                    expected,
                    actual
                )),
                GraphqlAssertionObservation::NoErrorsFailed { errors } => Some(format!(
                    "GraphQL operation {} at {} returned {} error(s)",
                    operation
                        .operation_name
                        .as_deref()
                        .unwrap_or("anonymous operation"),
                    operation.url,
                    errors.len()
                )),
                GraphqlAssertionObservation::StatusPassed { .. }
                | GraphqlAssertionObservation::NoErrorsPassed => None,
            })
            .unwrap_or_else(|| {
                format!(
                    "GraphQL effect failed during workflow run: {} returned status {}",
                    operation.url, response.status
                )
            }),
        Observation::GraphqlFailed {
            operation,
            message,
            status,
        } => format!(
            "GraphQL effect failed during workflow run: {}{}: {}",
            operation.url,
            status
                .map(|status| format!(" returned status {status}"))
                .unwrap_or_default(),
            message
        ),
        _ => "workflow run failed".to_string(),
    }
}

fn evaluate_rest_assertions(
    rest: &RestEffect,
    actual_status: u16,
) -> Vec<RestAssertionObservation> {
    rest.expect
        .status
        .map(|expected| {
            if expected == actual_status {
                RestAssertionObservation::StatusPassed {
                    expected,
                    actual: actual_status,
                }
            } else {
                RestAssertionObservation::StatusFailed {
                    expected,
                    actual: actual_status,
                }
            }
        })
        .into_iter()
        .collect()
}

fn evaluate_graphql_assertions(
    graphql: &GraphqlEffect,
    response: &GraphqlResponseObservation,
) -> Vec<GraphqlAssertionObservation> {
    let mut assertions = Vec::new();

    if let Some(expected) = graphql.expect.status {
        if expected == response.status {
            assertions.push(GraphqlAssertionObservation::StatusPassed {
                expected,
                actual: response.status,
            });
        } else {
            assertions.push(GraphqlAssertionObservation::StatusFailed {
                expected,
                actual: response.status,
            });
        }
    }

    if response.errors.is_empty() {
        assertions.push(GraphqlAssertionObservation::NoErrorsPassed);
    } else {
        assertions.push(GraphqlAssertionObservation::NoErrorsFailed {
            errors: response.errors.clone(),
        });
    }

    assertions
}

fn lookup_payload_path<'a>(payload: &'a Value, path: &str) -> Option<&'a Value> {
    let trimmed = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix("payload."))
        .unwrap_or(path);

    if trimmed == "$" || trimmed == "payload" || trimmed.is_empty() {
        return Some(payload);
    }

    let mut current = payload;
    for segment in trimmed.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn rest_extraction_document(response: &RestResponseObservation) -> Value {
    let body = match &response.body {
        RestBody::Json { value } => value.clone(),
        RestBody::Text { value } => Value::String(value.clone()),
        RestBody::Empty => Value::Null,
    };

    serde_json::json!({
        "status": response.status,
        "headers": response.headers,
        "body": body,
    })
}

fn graphql_extraction_document(response: &GraphqlResponseObservation) -> Value {
    serde_json::json!({
        "status": response.status,
        "headers": response.headers,
        "data": response.data,
        "errors": response.errors,
        "extensions": response.extensions,
    })
}

fn select_json_path_value(
    document: &Value,
    selector: &JsonPathSelector,
) -> Result<Option<Value>, String> {
    let query = jsonpath_rfc9535::JsonPath::parse(&selector.from)
        .map_err(|error| format!("failed to parse JSONPath '{}': {error}", selector.from))?;
    let values = query.query_values(document);

    Ok(match values.as_slice() {
        [] => None,
        [value] => Some((*value).clone()),
        values => Some(Value::Array(
            values.iter().map(|value| (*value).clone()).collect(),
        )),
    })
}

#[derive(Debug)]
struct BuiltRestRequest {
    operation: String,
    method: String,
    host: String,
    port: u16,
    path_and_query: String,
    headers: BTreeMap<String, String>,
    body: Option<String>,
    url: String,
}

fn build_rest_request(
    rest: &RestEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> Result<BuiltRestRequest, EngineError> {
    let operation = rest
        .operation
        .clone()
        .unwrap_or_else(|| format!("{} {}", rest.method.as_str(), rest.path));
    let base_url_template = rest
        .base_url
        .clone()
        .or_else(|| {
            rest.service
                .as_ref()
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .ok_or_else(|| {
            rest.service
                .as_ref()
                .map(|service| EngineError::MissingRestServiceBinding {
                    service: service.clone(),
                })
                .unwrap_or(EngineError::MissingRestBaseUrl)
        })?;

    let base_url = interpolate_string(&base_url_template, event, environment, &operation)?;
    let base = parse_http_base_url(&base_url)?;
    let path = interpolate_string(&rest.path, event, environment, &operation)?;
    let mut query_pairs = Vec::new();
    for (key, value) in &rest.query {
        query_pairs.push(format!(
            "{}={}",
            percent_encode(key),
            percent_encode(&interpolate_string(value, event, environment, &operation)?)
        ));
    }

    let path_and_query = if query_pairs.is_empty() {
        path.clone()
    } else {
        format!("{path}?{}", query_pairs.join("&"))
    };
    let url = format!("{}{}", base.origin, path_and_query);

    let mut headers = BTreeMap::new();
    for (key, value) in &rest.headers {
        headers.insert(
            key.clone(),
            interpolate_string(value, event, environment, &operation)?,
        );
    }

    let body = rest
        .json_body
        .as_ref()
        .map(|body| interpolate_json(body, event, environment, &operation))
        .transpose()?
        .map(|body| serde_json::to_string(&body).expect("JSON values serialize"));

    Ok(BuiltRestRequest {
        operation,
        method: rest.method.as_str().to_string(),
        host: base.host,
        port: base.port,
        path_and_query,
        headers,
        body,
        url,
    })
}

fn fallback_rest_operation(
    rest: &RestEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> RestOperationObservation {
    let method = rest.method.as_str().to_string();
    let base_url = rest
        .base_url
        .clone()
        .or_else(|| {
            rest.service
                .as_ref()
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .unwrap_or_else(|| "<unbound>".to_string());
    let path = interpolate_string(
        &rest.path,
        event,
        environment,
        rest.operation.as_deref().unwrap_or("rest"),
    )
    .unwrap_or_else(|_| rest.path.clone());

    RestOperationObservation {
        service: rest.service.clone(),
        method,
        url: format!("{}{}", base_url.trim_end_matches('/'), path),
    }
}

fn build_graphql_request(
    graphql: &GraphqlEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> Result<BuiltRestRequest, EngineError> {
    let operation = graphql
        .operation_name
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let base_url_template = graphql
        .base_url
        .clone()
        .or_else(|| {
            graphql
                .service
                .as_ref()
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .ok_or_else(|| {
            graphql
                .service
                .as_ref()
                .map(|service| EngineError::MissingGraphqlServiceBinding {
                    service: service.clone(),
                })
                .unwrap_or(EngineError::MissingGraphqlBaseUrl)
        })?;

    let base_url = interpolate_string(&base_url_template, event, environment, &operation)
        .map_err(|error| graphql_build_error(&operation, error))?;
    let base = parse_graphql_base_url(&base_url)?;
    let query = interpolate_string(&graphql.query, event, environment, &operation)
        .map_err(|error| graphql_build_error(&operation, error))?;
    let variables = interpolate_json(&graphql.variables, event, environment, &operation)
        .map_err(|error| graphql_build_error(&operation, error))?;

    let mut body = serde_json::Map::new();
    body.insert("query".to_string(), Value::String(query));
    body.insert("variables".to_string(), variables);
    if let Some(operation_name) = &graphql.operation_name {
        body.insert(
            "operationName".to_string(),
            Value::String(operation_name.clone()),
        );
    }

    Ok(BuiltRestRequest {
        operation,
        method: "POST".to_string(),
        host: base.host,
        port: base.port,
        path_and_query: base.path,
        headers: BTreeMap::new(),
        body: Some(serde_json::to_string(&Value::Object(body)).expect("JSON values serialize")),
        url: base.url,
    })
}

fn fallback_graphql_operation(
    graphql: &GraphqlEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> GraphqlOperationObservation {
    let base_url = graphql
        .base_url
        .clone()
        .or_else(|| {
            graphql
                .service
                .as_ref()
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .unwrap_or_else(|| "<unbound>".to_string());
    let url = interpolate_string(
        &base_url,
        event,
        environment,
        graphql.operation_name.as_deref().unwrap_or("graphql"),
    )
    .unwrap_or(base_url);

    GraphqlOperationObservation {
        service: graphql.service.clone(),
        operation_name: graphql.operation_name.clone(),
        url,
    }
}

fn graphql_build_error(operation: &str, error: EngineError) -> EngineError {
    EngineError::GraphqlRequestBuild {
        operation: operation.to_string(),
        message: error.to_string(),
    }
}

#[derive(Debug)]
struct HttpBaseUrl {
    origin: String,
    host: String,
    port: u16,
}

#[derive(Debug)]
struct GraphqlBaseUrl {
    host: String,
    port: u16,
    path: String,
    url: String,
}

fn parse_http_base_url(base_url: &str) -> Result<HttpBaseUrl, EngineError> {
    let without_scheme =
        base_url
            .strip_prefix("http://")
            .ok_or_else(|| EngineError::UnsupportedRestBaseUrl {
                base_url: base_url.to_string(),
            })?;
    let authority = without_scheme.trim_end_matches('/');
    if authority.contains('/') {
        return Err(EngineError::UnsupportedRestBaseUrl {
            base_url: base_url.to_string(),
        });
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_string(),
            port.parse::<u16>()
                .map_err(|error| EngineError::UnsupportedRestBaseUrl {
                    base_url: format!("{base_url} ({error})"),
                })?,
        ),
        None => (authority.to_string(), 80),
    };

    Ok(HttpBaseUrl {
        origin: format!("http://{}:{}", host, port),
        host,
        port,
    })
}

fn parse_graphql_base_url(base_url: &str) -> Result<GraphqlBaseUrl, EngineError> {
    let without_scheme =
        base_url
            .strip_prefix("http://")
            .ok_or_else(|| EngineError::UnsupportedGraphqlBaseUrl {
                base_url: base_url.to_string(),
            })?;
    let (authority, path) = without_scheme
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or((without_scheme, "/graphql".to_string()));
    let authority = authority.trim_end_matches('/');
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_string(),
            port.parse::<u16>()
                .map_err(|error| EngineError::UnsupportedGraphqlBaseUrl {
                    base_url: format!("{base_url} ({error})"),
                })?,
        ),
        None => (authority.to_string(), 80),
    };

    Ok(GraphqlBaseUrl {
        host: host.clone(),
        port,
        path: path.clone(),
        url: format!("http://{}:{}{}", host, port, path),
    })
}

fn execute_http_request(
    request: &BuiltRestRequest,
) -> Result<RestResponseObservation, EngineError> {
    let mut stream =
        TcpStream::connect((request.host.as_str(), request.port)).map_err(|error| {
            EngineError::RestRequestFailed {
                operation: request.operation.clone(),
                message: error.to_string(),
            }
        })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| EngineError::RestRequestFailed {
            operation: request.operation.clone(),
            message: error.to_string(),
        })?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| EngineError::RestRequestFailed {
            operation: request.operation.clone(),
            message: error.to_string(),
        })?;

    let body = request.body.as_deref().unwrap_or("");
    let mut http_request = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n",
        request.method, request.path_and_query, request.host
    );
    for (key, value) in &request.headers {
        http_request.push_str(&format!("{key}: {value}\r\n"));
    }
    if request.body.is_some() {
        http_request.push_str("Content-Type: application/json\r\n");
    }
    http_request.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));

    stream
        .write_all(http_request.as_bytes())
        .map_err(|error| EngineError::RestRequestFailed {
            operation: request.operation.clone(),
            message: error.to_string(),
        })?;

    let mut raw_response = Vec::new();
    stream
        .read_to_end(&mut raw_response)
        .map_err(|error| EngineError::RestRequestFailed {
            operation: request.operation.clone(),
            message: error.to_string(),
        })?;

    parse_http_response(&request.operation, &raw_response)
}

fn parse_http_response(
    operation: &str,
    raw_response: &[u8],
) -> Result<RestResponseObservation, EngineError> {
    let response = String::from_utf8_lossy(raw_response);
    let (head, body) =
        response
            .split_once("\r\n\r\n")
            .ok_or_else(|| EngineError::RestRequestFailed {
                operation: operation.to_string(),
                message: "malformed HTTP response".to_string(),
            })?;
    let mut lines = head.lines();
    let status_line = lines.next().ok_or_else(|| EngineError::RestRequestFailed {
        operation: operation.to_string(),
        message: "missing HTTP status line".to_string(),
    })?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| EngineError::RestRequestFailed {
            operation: operation.to_string(),
            message: "missing HTTP status code".to_string(),
        })?
        .parse::<u16>()
        .map_err(|error| EngineError::RestRequestFailed {
            operation: operation.to_string(),
            message: error.to_string(),
        })?;

    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let body = if body.is_empty() {
        RestBody::Empty
    } else if headers
        .get("content-type")
        .is_some_and(|content_type| content_type.contains("application/json"))
    {
        match serde_json::from_str::<Value>(body) {
            Ok(value) => RestBody::Json { value },
            Err(_) => RestBody::Text {
                value: body.to_string(),
            },
        }
    } else {
        RestBody::Text {
            value: body.to_string(),
        }
    };

    Ok(RestResponseObservation {
        status,
        headers,
        body,
    })
}

fn graphql_response_from_http(
    operation: &str,
    response: RestResponseObservation,
) -> Result<GraphqlResponseObservation, EngineError> {
    let RestBody::Json { value } = response.body else {
        return Err(EngineError::GraphqlRequestFailed {
            operation: operation.to_string(),
            message: "GraphQL response body was not JSON".to_string(),
        });
    };

    let data = value.get("data").cloned();
    let errors = value
        .get("errors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let extensions = value.get("extensions").cloned();

    Ok(GraphqlResponseObservation {
        status: response.status,
        headers: response.headers,
        data,
        errors,
        extensions,
    })
}

fn interpolate_json(
    value: &Value,
    event: &Event,
    environment: &RuntimeEnvironment,
    operation: &str,
) -> Result<Value, EngineError> {
    match value {
        Value::String(value) => Ok(Value::String(interpolate_string(
            value,
            event,
            environment,
            operation,
        )?)),
        Value::Array(values) => values
            .iter()
            .map(|value| interpolate_json(value, event, environment, operation))
            .collect(),
        Value::Object(object) => object
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    interpolate_json(value, event, environment, operation)?,
                ))
            })
            .collect::<Result<serde_json::Map<String, Value>, EngineError>>()
            .map(Value::Object),
        _ => Ok(value.clone()),
    }
}

fn interpolate_string(
    template: &str,
    event: &Event,
    environment: &RuntimeEnvironment,
    operation: &str,
) -> Result<String, EngineError> {
    let mut output = String::new();
    let mut remaining = template;

    while let Some(start) = remaining.find("{{") {
        let before = &remaining[..start];
        output.push_str(before);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("}}") else {
            return Err(EngineError::RestRequestBuild {
                operation: operation.to_string(),
                message: format!("unclosed template in '{template}'"),
            });
        };
        let expression = after_start[..end].trim();
        output.push_str(&resolve_template_expression(
            expression,
            event,
            environment,
            operation,
        )?);
        remaining = &after_start[end + 2..];
    }

    output.push_str(remaining);
    Ok(output)
}

fn resolve_template_expression(
    expression: &str,
    event: &Event,
    environment: &RuntimeEnvironment,
    operation: &str,
) -> Result<String, EngineError> {
    if let Some(path) = expression.strip_prefix("event.payload.") {
        return lookup_payload_path(&event.payload, path)
            .map(template_value_to_string)
            .ok_or_else(|| EngineError::RestRequestBuild {
                operation: operation.to_string(),
                message: format!("event payload path '{path}' was not found"),
            });
    }

    if let Some(name) = expression.strip_prefix("env.") {
        return environment.values.get(name).cloned().ok_or_else(|| {
            EngineError::RestRequestBuild {
                operation: operation.to_string(),
                message: format!("environment value '{name}' was not found"),
            }
        });
    }

    Err(EngineError::RestRequestBuild {
        operation: operation.to_string(),
        message: format!("unsupported template expression '{expression}'"),
    })
}

fn template_value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

struct RunState {
    run_id: String,
    workflow_name: String,
    trace: Vec<TraceEntry>,
    next_trace_sequence: u64,
}

impl RunState {
    fn new(run_id: String, workflow_name: String) -> Self {
        Self {
            run_id,
            workflow_name,
            trace: Vec::new(),
            next_trace_sequence: 0,
        }
    }

    fn next_trace_id(&mut self) -> String {
        self.next_trace_sequence += 1;
        format!("trace-{}", self.next_trace_sequence)
    }

    fn push_trace(&mut self, kind: TraceEntryKind) -> String {
        let id = self.next_trace_id();
        self.push_trace_with_id(id.clone(), kind);
        id
    }

    fn push_trace_with_id(&mut self, id: String, kind: TraceEntryKind) {
        self.trace.push(TraceEntry {
            sequence: self.next_trace_sequence,
            id,
            kind,
        });
    }

    fn succeed(mut self) -> RunReport {
        self.push_trace(TraceEntryKind::RunEnded {
            status: RunStatus::Succeeded,
        });
        RunReport {
            run_id: self.run_id,
            workflow_name: self.workflow_name,
            status: RunStatus::Succeeded,
            trace: self.trace,
            failure: None,
        }
    }

    fn fail(mut self, trace_entry_id: Option<String>, message: String) -> RunReport {
        self.push_trace(TraceEntryKind::RunEnded {
            status: RunStatus::Failed,
        });
        RunReport {
            run_id: self.run_id,
            workflow_name: self.workflow_name,
            status: RunStatus::Failed,
            trace: self.trace,
            failure: Some(TraceFailure {
                trace_entry_id,
                message,
            }),
        }
    }
}
