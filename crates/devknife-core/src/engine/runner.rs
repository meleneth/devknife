use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

use quick_xml::{events::Event as XmlEvent, Reader};
use serde_json::Value;
use tungstenite::{client, Message};
use uuid::Uuid;

use crate::domain::{
    AssertEffect, AwsOperationObservation, Effect, Event, EventCause, GraphqlAssertionObservation,
    GraphqlEffect, GraphqlOperationObservation, GraphqlResponseObservation, JsonPathSelector,
    Observation, RestAssertionObservation, RestBody, RestEffect, RestOperationObservation,
    RestResponseObservation, RunReport, RunStatus, RuntimeEnvironment, SnsPublishEffect,
    SqsMessageObservation, SqsReceiveEffect, SqsSendEffect, TraceEntry, TraceEntryKind,
    TraceFailure, WebsocketAssertionObservation, WebsocketEffect, WebsocketOperationObservation,
    WebsocketReceivedObservation, WebsocketSend, WebsocketSentObservation, Workflow,
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
                        Observation::SnsPublish { emitted_events, .. }
                        | Observation::SqsSend { emitted_events, .. }
                        | Observation::SqsReceive { emitted_events, .. }
                        | Observation::WebsocketMessage { emitted_events, .. } => {
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
            Effect::SnsPublish(sns) => {
                self.execute_sns_publish_effect(sns, event, trace_entry_id, created_events)
            }
            Effect::SqsSend(sqs) => {
                self.execute_sqs_send_effect(sqs, event, trace_entry_id, created_events)
            }
            Effect::SqsReceive(sqs) => {
                self.execute_sqs_receive_effect(sqs, event, trace_entry_id, created_events)
            }
            Effect::Websocket(websocket) => {
                self.execute_websocket_effect(websocket, event, trace_entry_id, created_events)
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

    fn execute_sns_publish_effect(
        &self,
        sns: &SnsPublishEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
    ) -> Result<Observation, EngineError> {
        let operation = AwsOperationObservation {
            service: sns.service.clone(),
            action: "Publish".to_string(),
            url: fallback_aws_endpoint_url(
                sns.service.as_deref(),
                sns.endpoint_url.as_deref(),
                &self.environment,
            ),
        };
        let request = match build_sns_publish_request(sns, event, &self.environment) {
            Ok(request) => request,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation,
                    message: error.to_string(),
                });
            }
        };
        let response = match execute_http_request(&request) {
            Ok(response) => response,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation: aws_operation_observation(
                        sns.service.clone(),
                        "Publish",
                        &request.url,
                    ),
                    message: error.to_string(),
                });
            }
        };
        let message_id = match parse_single_xml_text(&request.operation, &response, "MessageId") {
            Ok(message_id) => message_id,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation: aws_operation_observation(
                        sns.service.clone(),
                        "Publish",
                        &request.url,
                    ),
                    message: error.to_string(),
                });
            }
        };
        let document = serde_json::json!({
            "message_id": message_id,
            "topic_arn": sns.topic_arn,
        });
        let emitted_events = self.build_aws_emitted_events(
            sns.emits
                .iter()
                .map(|emit| (&emit.event_type, &emit.payload)),
            event,
            trace_entry_id,
            created_events,
            &document,
        )?;

        Ok(Observation::SnsPublish {
            operation: aws_operation_observation(sns.service.clone(), "Publish", &request.url),
            message_id,
            emitted_events,
        })
    }

    fn execute_sqs_send_effect(
        &self,
        sqs: &SqsSendEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
    ) -> Result<Observation, EngineError> {
        let operation = AwsOperationObservation {
            service: sqs.service.clone(),
            action: "SendMessage".to_string(),
            url: fallback_aws_endpoint_url(
                sqs.service.as_deref(),
                sqs.endpoint_url.as_deref(),
                &self.environment,
            ),
        };
        let request = match build_sqs_send_request(sqs, event, &self.environment) {
            Ok(request) => request,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation,
                    message: error.to_string(),
                });
            }
        };
        let response = match execute_http_request(&request) {
            Ok(response) => response,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation: aws_operation_observation(
                        sqs.service.clone(),
                        "SendMessage",
                        &request.url,
                    ),
                    message: error.to_string(),
                });
            }
        };
        let message_id = match parse_single_xml_text(&request.operation, &response, "MessageId") {
            Ok(message_id) => message_id,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation: aws_operation_observation(
                        sqs.service.clone(),
                        "SendMessage",
                        &request.url,
                    ),
                    message: error.to_string(),
                });
            }
        };
        let document = serde_json::json!({
            "message_id": message_id,
            "queue_url": sqs.queue_url,
        });
        let emitted_events = self.build_aws_emitted_events(
            sqs.emits
                .iter()
                .map(|emit| (&emit.event_type, &emit.payload)),
            event,
            trace_entry_id,
            created_events,
            &document,
        )?;

        Ok(Observation::SqsSend {
            operation: aws_operation_observation(sqs.service.clone(), "SendMessage", &request.url),
            message_id,
            emitted_events,
        })
    }

    fn execute_sqs_receive_effect(
        &self,
        sqs: &SqsReceiveEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
    ) -> Result<Observation, EngineError> {
        let operation = AwsOperationObservation {
            service: sqs.service.clone(),
            action: "ReceiveMessage".to_string(),
            url: fallback_aws_endpoint_url(
                sqs.service.as_deref(),
                sqs.endpoint_url.as_deref(),
                &self.environment,
            ),
        };
        let request = match build_sqs_receive_request(sqs, event, &self.environment) {
            Ok(request) => request,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation,
                    message: error.to_string(),
                });
            }
        };
        let response = match execute_http_request(&request) {
            Ok(response) => response,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation: aws_operation_observation(
                        sqs.service.clone(),
                        "ReceiveMessage",
                        &request.url,
                    ),
                    message: error.to_string(),
                });
            }
        };
        let messages = match parse_sqs_receive_messages(&request.operation, &response) {
            Ok(messages) => messages,
            Err(error) => {
                return Ok(Observation::AwsFailed {
                    operation: aws_operation_observation(
                        sqs.service.clone(),
                        "ReceiveMessage",
                        &request.url,
                    ),
                    message: error.to_string(),
                });
            }
        };
        let mut deleted_receipt_handles = Vec::new();
        if sqs.delete_on_success {
            for message in &messages {
                let delete_request = match build_sqs_delete_request(
                    sqs,
                    event,
                    &self.environment,
                    &message.receipt_handle,
                ) {
                    Ok(request) => request,
                    Err(error) => {
                        return Ok(Observation::AwsFailed {
                            operation: aws_operation_observation(
                                sqs.service.clone(),
                                "DeleteMessage",
                                &request.url,
                            ),
                            message: error.to_string(),
                        });
                    }
                };
                if let Err(error) = execute_http_request(&delete_request) {
                    return Ok(Observation::AwsFailed {
                        operation: aws_operation_observation(
                            sqs.service.clone(),
                            "DeleteMessage",
                            &delete_request.url,
                        ),
                        message: error.to_string(),
                    });
                }
                deleted_receipt_handles.push(message.receipt_handle.clone());
            }
        }

        let document = sqs_receive_extraction_document(&messages);
        let emitted_events = self.build_aws_emitted_events(
            sqs.emits
                .iter()
                .map(|emit| (&emit.event_type, &emit.payload)),
            event,
            trace_entry_id,
            created_events,
            &document,
        )?;

        Ok(Observation::SqsReceive {
            operation: aws_operation_observation(
                sqs.service.clone(),
                "ReceiveMessage",
                &request.url,
            ),
            messages,
            deleted_receipt_handles,
            emitted_events,
        })
    }

    fn build_aws_emitted_events<'a>(
        &self,
        emissions: impl Iterator<Item = (&'a String, &'a BTreeMap<String, JsonPathSelector>)>,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
        document: &Value,
    ) -> Result<Vec<Event>, EngineError> {
        let mut emitted_events = Vec::new();

        for (event_type, mapping) in emissions {
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
            for (field, path) in mapping {
                let value = select_json_path_value(document, path).map_err(|error| {
                    EngineError::AwsEmissionPathInvalid {
                        event_type: event_type.clone(),
                        path: path.from.clone(),
                        message: error,
                    }
                })?;
                let value = value.ok_or_else(|| EngineError::AwsEmissionPathMissing {
                    event_type: event_type.clone(),
                    path: path.from.clone(),
                })?;
                payload.insert(field.clone(), value);
            }

            *created_events += 1;
            emitted_events.push(Event {
                id: format!("event-{}", created_events),
                event_type: event_type.clone(),
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

    fn execute_websocket_effect(
        &self,
        websocket: &WebsocketEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
    ) -> Result<Observation, EngineError> {
        let operation = match build_websocket_operation(websocket, event, &self.environment) {
            Ok(operation) => operation,
            Err(error) => {
                return Ok(Observation::WebsocketFailed {
                    operation: WebsocketOperationObservation {
                        service: websocket.service.clone(),
                        session: websocket.session.clone(),
                        url: fallback_websocket_url(
                            websocket.service.as_deref(),
                            websocket.url.as_deref(),
                            &self.environment,
                        ),
                    },
                    message: error.to_string(),
                });
            }
        };
        let parsed_url = match parse_websocket_url(&operation.url) {
            Ok(parsed_url) => parsed_url,
            Err(error) => {
                return Ok(Observation::WebsocketFailed {
                    operation,
                    message: error.to_string(),
                });
            }
        };
        let timeout = Duration::from_millis(websocket.timeout_ms);
        let stream = match TcpStream::connect((parsed_url.host.as_str(), parsed_url.port)) {
            Ok(stream) => stream,
            Err(error) => {
                return Ok(Observation::WebsocketFailed {
                    operation,
                    message: error.to_string(),
                });
            }
        };
        if let Err(error) = stream.set_read_timeout(Some(timeout)) {
            return Ok(Observation::WebsocketFailed {
                operation,
                message: error.to_string(),
            });
        }
        if let Err(error) = stream.set_write_timeout(Some(timeout)) {
            return Ok(Observation::WebsocketFailed {
                operation,
                message: error.to_string(),
            });
        }

        let (mut socket, _) = match client(operation.url.as_str(), stream) {
            Ok(connection) => connection,
            Err(error) => {
                return Ok(Observation::WebsocketFailed {
                    operation,
                    message: error.to_string(),
                });
            }
        };
        let (send_message, sent) =
            match build_websocket_send_message(&websocket.send, event, &self.environment) {
                Ok(send) => send,
                Err(error) => {
                    return Ok(Observation::WebsocketFailed {
                        operation,
                        message: error.to_string(),
                    });
                }
            };
        if let Err(error) = socket.send(send_message) {
            return Ok(Observation::WebsocketFailed {
                operation,
                message: error.to_string(),
            });
        }

        let received_message = match socket.read() {
            Ok(message) => message,
            Err(error) => {
                return Ok(Observation::WebsocketFailed {
                    operation,
                    message: error.to_string(),
                });
            }
        };
        let (received, document) = websocket_received_observation(received_message);
        let assertions = evaluate_websocket_assertions(websocket, &document);
        let emitted_events = self.build_websocket_emitted_events(
            websocket,
            event,
            trace_entry_id,
            created_events,
            &document,
        )?;
        let _ = socket.close(None);

        Ok(Observation::WebsocketMessage {
            operation,
            sent,
            received,
            assertions,
            emitted_events,
        })
    }

    fn build_websocket_emitted_events(
        &self,
        websocket: &WebsocketEffect,
        event: &Event,
        trace_entry_id: &str,
        created_events: &mut usize,
        document: &Value,
    ) -> Result<Vec<Event>, EngineError> {
        let mut emitted_events = Vec::new();

        for emission in &websocket.emits {
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
                let value = select_json_path_value(document, path).map_err(|error| {
                    EngineError::WebsocketEmissionPathInvalid {
                        event_type: emission.event_type.clone(),
                        path: path.from.clone(),
                        message: error,
                    }
                })?;
                let value = value.ok_or_else(|| EngineError::WebsocketEmissionPathMissing {
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
        Observation::AwsFailed { .. } => true,
        Observation::WebsocketMessage { assertions, .. } => assertions.iter().any(|assertion| {
            matches!(
                assertion,
                WebsocketAssertionObservation::JsonFieldFailed { .. }
            )
        }),
        Observation::WebsocketFailed { .. } => true,
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
        Observation::AwsFailed { operation, message } => format!(
            "AWS effect failed during workflow run: {} {}: {}",
            operation.action, operation.url, message
        ),
        Observation::WebsocketMessage {
            operation,
            assertions,
            ..
        } => assertions
            .iter()
            .find_map(|assertion| match assertion {
                WebsocketAssertionObservation::JsonFieldFailed {
                    path,
                    expected,
                    actual,
                } => Some(format!(
                    "WebSocket assertion failed during workflow run: {} expected {}, actual {}",
                    path,
                    expected,
                    actual
                        .as_ref()
                        .map(Value::to_string)
                        .unwrap_or_else(|| "<missing>".to_string())
                )),
                WebsocketAssertionObservation::JsonFieldPassed { .. } => None,
            })
            .unwrap_or_else(|| {
                format!(
                    "WebSocket effect failed during workflow run: {}",
                    operation.url
                )
            }),
        Observation::WebsocketFailed { operation, message } => format!(
            "WebSocket effect failed during workflow run: {}: {}",
            operation.url, message
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
    content_type: Option<String>,
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
        content_type: rest
            .json_body
            .as_ref()
            .map(|_| "application/json".to_string()),
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
        content_type: Some("application/json".to_string()),
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

fn build_sns_publish_request(
    sns: &SnsPublishEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> Result<BuiltRestRequest, EngineError> {
    let action = "Publish";
    let endpoint = build_aws_endpoint_url(
        sns.service.as_deref(),
        sns.endpoint_url.as_deref(),
        event,
        environment,
        action,
    )?;
    let topic_arn = interpolate_string(&sns.topic_arn, event, environment, action)
        .map_err(|error| aws_build_error(action, error))?;
    let message = interpolate_json(&sns.message, event, environment, action)
        .map_err(|error| aws_build_error(action, error))?;
    let message = serde_json::to_string(&message).expect("JSON values serialize");
    let params = vec![
        ("Action".to_string(), action.to_string()),
        ("Version".to_string(), "2010-03-31".to_string()),
        ("TopicArn".to_string(), topic_arn),
        ("Message".to_string(), message),
    ];

    build_aws_query_request(action, endpoint, "/", params)
}

fn build_sqs_send_request(
    sqs: &SqsSendEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> Result<BuiltRestRequest, EngineError> {
    let action = "SendMessage";
    let endpoint = build_aws_endpoint_url(
        sqs.service.as_deref(),
        sqs.endpoint_url.as_deref(),
        event,
        environment,
        action,
    )?;
    let queue_path = aws_queue_path(&sqs.queue_url, event, environment, action)?;
    let message = interpolate_json(&sqs.message, event, environment, action)
        .map_err(|error| aws_build_error(action, error))?;
    let message = serde_json::to_string(&message).expect("JSON values serialize");
    let params = vec![
        ("Action".to_string(), action.to_string()),
        ("Version".to_string(), "2012-11-05".to_string()),
        ("MessageBody".to_string(), message),
    ];

    build_aws_query_request(action, endpoint, &queue_path, params)
}

fn build_sqs_receive_request(
    sqs: &SqsReceiveEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> Result<BuiltRestRequest, EngineError> {
    let action = "ReceiveMessage";
    let endpoint = build_aws_endpoint_url(
        sqs.service.as_deref(),
        sqs.endpoint_url.as_deref(),
        event,
        environment,
        action,
    )?;
    let queue_path = aws_queue_path(&sqs.queue_url, event, environment, action)?;
    let params = vec![
        ("Action".to_string(), action.to_string()),
        ("Version".to_string(), "2012-11-05".to_string()),
        (
            "MaxNumberOfMessages".to_string(),
            sqs.max_messages.to_string(),
        ),
        (
            "WaitTimeSeconds".to_string(),
            sqs.wait_time_seconds.to_string(),
        ),
    ];

    build_aws_query_request(action, endpoint, &queue_path, params)
}

fn build_sqs_delete_request(
    sqs: &SqsReceiveEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
    receipt_handle: &str,
) -> Result<BuiltRestRequest, EngineError> {
    let action = "DeleteMessage";
    let endpoint = build_aws_endpoint_url(
        sqs.service.as_deref(),
        sqs.endpoint_url.as_deref(),
        event,
        environment,
        action,
    )?;
    let queue_path = aws_queue_path(&sqs.queue_url, event, environment, action)?;
    let params = vec![
        ("Action".to_string(), action.to_string()),
        ("Version".to_string(), "2012-11-05".to_string()),
        ("ReceiptHandle".to_string(), receipt_handle.to_string()),
    ];

    build_aws_query_request(action, endpoint, &queue_path, params)
}

fn build_aws_query_request(
    action: &str,
    endpoint: AwsEndpointUrl,
    path: &str,
    params: Vec<(String, String)>,
) -> Result<BuiltRestRequest, EngineError> {
    let body = params
        .into_iter()
        .map(|(key, value)| format!("{}={}", percent_encode(&key), percent_encode(&value)))
        .collect::<Vec<_>>()
        .join("&");
    let path = if path.is_empty() { "/" } else { path };
    let url = format!("{}{}", endpoint.origin, path);

    Ok(BuiltRestRequest {
        operation: action.to_string(),
        method: "POST".to_string(),
        host: endpoint.host,
        port: endpoint.port,
        path_and_query: path.to_string(),
        headers: BTreeMap::new(),
        body: Some(body),
        content_type: Some("application/x-www-form-urlencoded".to_string()),
        url,
    })
}

fn build_aws_endpoint_url(
    service: Option<&str>,
    endpoint_url: Option<&str>,
    event: &Event,
    environment: &RuntimeEnvironment,
    action: &str,
) -> Result<AwsEndpointUrl, EngineError> {
    let endpoint_template = endpoint_url
        .map(str::to_string)
        .or_else(|| {
            service
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .ok_or_else(|| {
            service
                .map(|service| EngineError::MissingAwsServiceBinding {
                    service: service.to_string(),
                })
                .unwrap_or(EngineError::MissingAwsEndpointUrl)
        })?;
    let endpoint_url = interpolate_string(&endpoint_template, event, environment, action)
        .map_err(|error| aws_build_error(action, error))?;
    parse_aws_endpoint_url(&endpoint_url)
}

fn fallback_aws_endpoint_url(
    service: Option<&str>,
    endpoint_url: Option<&str>,
    environment: &RuntimeEnvironment,
) -> String {
    endpoint_url
        .map(str::to_string)
        .or_else(|| {
            service
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .unwrap_or_else(|| "<unbound>".to_string())
}

fn aws_queue_path(
    queue_url: &str,
    event: &Event,
    environment: &RuntimeEnvironment,
    action: &str,
) -> Result<String, EngineError> {
    let queue_url = interpolate_string(queue_url, event, environment, action)
        .map_err(|error| aws_build_error(action, error))?;
    if let Some(without_scheme) = queue_url.strip_prefix("http://") {
        let (_, path) =
            without_scheme
                .split_once('/')
                .ok_or_else(|| EngineError::AwsRequestBuild {
                    action: action.to_string(),
                    message: format!("queue URL '{queue_url}' does not include a path"),
                })?;
        return Ok(format!("/{path}"));
    }

    if queue_url.starts_with('/') {
        Ok(queue_url)
    } else {
        Err(EngineError::AwsRequestBuild {
            action: action.to_string(),
            message: format!("queue URL '{queue_url}' must be an http:// URL or absolute path"),
        })
    }
}

fn aws_build_error(action: &str, error: EngineError) -> EngineError {
    EngineError::AwsRequestBuild {
        action: action.to_string(),
        message: error.to_string(),
    }
}

fn aws_operation_observation(
    service: Option<String>,
    action: &str,
    url: &str,
) -> AwsOperationObservation {
    AwsOperationObservation {
        service,
        action: action.to_string(),
        url: url.to_string(),
    }
}

#[derive(Debug)]
struct AwsEndpointUrl {
    origin: String,
    host: String,
    port: u16,
}

fn parse_aws_endpoint_url(endpoint_url: &str) -> Result<AwsEndpointUrl, EngineError> {
    let without_scheme = endpoint_url.strip_prefix("http://").ok_or_else(|| {
        EngineError::UnsupportedAwsEndpointUrl {
            endpoint_url: endpoint_url.to_string(),
        }
    })?;
    let authority = without_scheme.trim_end_matches('/');
    if authority.contains('/') {
        return Err(EngineError::UnsupportedAwsEndpointUrl {
            endpoint_url: endpoint_url.to_string(),
        });
    }

    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_string(),
            port.parse::<u16>()
                .map_err(|error| EngineError::UnsupportedAwsEndpointUrl {
                    endpoint_url: format!("{endpoint_url} ({error})"),
                })?,
        ),
        None => (authority.to_string(), 80),
    };

    Ok(AwsEndpointUrl {
        origin: format!("http://{}:{}", host, port),
        host,
        port,
    })
}

fn build_websocket_operation(
    websocket: &WebsocketEffect,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> Result<WebsocketOperationObservation, EngineError> {
    let url_template = websocket
        .url
        .clone()
        .or_else(|| {
            websocket
                .service
                .as_ref()
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .ok_or_else(|| {
            websocket
                .service
                .as_ref()
                .map(|service| EngineError::MissingWebsocketServiceBinding {
                    service: service.clone(),
                })
                .unwrap_or(EngineError::MissingWebsocketUrl)
        })?;
    let operation = websocket
        .session
        .as_deref()
        .unwrap_or("websocket")
        .to_string();
    let url =
        interpolate_string(&url_template, event, environment, &operation).map_err(|error| {
            EngineError::WebsocketRequestBuild {
                operation: operation.clone(),
                message: error.to_string(),
            }
        })?;

    Ok(WebsocketOperationObservation {
        service: websocket.service.clone(),
        session: websocket.session.clone(),
        url,
    })
}

fn fallback_websocket_url(
    service: Option<&str>,
    url: Option<&str>,
    environment: &RuntimeEnvironment,
) -> String {
    url.map(str::to_string)
        .or_else(|| {
            service
                .and_then(|service| environment.services.get(service))
                .map(|binding| binding.base_url.clone())
        })
        .unwrap_or_else(|| "<unbound>".to_string())
}

#[derive(Debug)]
struct ParsedWebsocketUrl {
    host: String,
    port: u16,
}

fn parse_websocket_url(url: &str) -> Result<ParsedWebsocketUrl, EngineError> {
    let without_scheme =
        url.strip_prefix("ws://")
            .ok_or_else(|| EngineError::UnsupportedWebsocketUrl {
                url: url.to_string(),
            })?;
    let authority = without_scheme
        .split_once('/')
        .map(|(authority, _)| authority)
        .unwrap_or(without_scheme);
    if authority.is_empty() {
        return Err(EngineError::UnsupportedWebsocketUrl {
            url: url.to_string(),
        });
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_string(),
            port.parse::<u16>()
                .map_err(|error| EngineError::UnsupportedWebsocketUrl {
                    url: format!("{url} ({error})"),
                })?,
        ),
        None => (authority.to_string(), 80),
    };

    Ok(ParsedWebsocketUrl { host, port })
}

fn build_websocket_send_message(
    send: &WebsocketSend,
    event: &Event,
    environment: &RuntimeEnvironment,
) -> Result<(Message, WebsocketSentObservation), EngineError> {
    match send {
        WebsocketSend::Json(value) => {
            let value =
                interpolate_json(value, event, environment, "websocket").map_err(|error| {
                    EngineError::WebsocketRequestBuild {
                        operation: "websocket".to_string(),
                        message: error.to_string(),
                    }
                })?;
            let text = serde_json::to_string(&value).expect("JSON values serialize");
            Ok((
                Message::text(text),
                WebsocketSentObservation::Json { value },
            ))
        }
        WebsocketSend::Text(value) => {
            let value =
                interpolate_string(value, event, environment, "websocket").map_err(|error| {
                    EngineError::WebsocketRequestBuild {
                        operation: "websocket".to_string(),
                        message: error.to_string(),
                    }
                })?;
            Ok((
                Message::text(value.clone()),
                WebsocketSentObservation::Text { value },
            ))
        }
    }
}

fn websocket_received_observation(message: Message) -> (WebsocketReceivedObservation, Value) {
    match message {
        Message::Text(text) => {
            let text = text.to_string();
            match serde_json::from_str::<Value>(&text) {
                Ok(value) => (
                    WebsocketReceivedObservation::Json {
                        value: value.clone(),
                    },
                    value,
                ),
                Err(_) => (
                    WebsocketReceivedObservation::Text {
                        value: text.clone(),
                    },
                    serde_json::json!({ "text": text }),
                ),
            }
        }
        Message::Binary(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            (
                WebsocketReceivedObservation::Text {
                    value: text.clone(),
                },
                serde_json::json!({ "text": text }),
            )
        }
        Message::Ping(bytes) | Message::Pong(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            (
                WebsocketReceivedObservation::Text {
                    value: text.clone(),
                },
                serde_json::json!({ "text": text }),
            )
        }
        Message::Close(close) => {
            let text = close
                .map(|close| close.reason.to_string())
                .unwrap_or_else(|| "close".to_string());
            (
                WebsocketReceivedObservation::Text {
                    value: text.clone(),
                },
                serde_json::json!({ "text": text }),
            )
        }
        Message::Frame(_) => (
            WebsocketReceivedObservation::Text {
                value: "frame".to_string(),
            },
            serde_json::json!({ "text": "frame" }),
        ),
    }
}

fn evaluate_websocket_assertions(
    websocket: &WebsocketEffect,
    document: &Value,
) -> Vec<WebsocketAssertionObservation> {
    websocket
        .expect
        .json
        .iter()
        .map(
            |(path, expected)| match jsonpath_rfc9535::JsonPath::parse(path) {
                Ok(query) => {
                    let values = query.query_values(document);
                    let actual = match values.as_slice() {
                        [] => None,
                        [value] => Some((*value).clone()),
                        values => Some(Value::Array(
                            values.iter().map(|value| (*value).clone()).collect(),
                        )),
                    };
                    if actual.as_ref() == Some(expected) {
                        WebsocketAssertionObservation::JsonFieldPassed { path: path.clone() }
                    } else {
                        WebsocketAssertionObservation::JsonFieldFailed {
                            path: path.clone(),
                            expected: expected.clone(),
                            actual,
                        }
                    }
                }
                Err(_) => WebsocketAssertionObservation::JsonFieldFailed {
                    path: path.clone(),
                    expected: expected.clone(),
                    actual: None,
                },
            },
        )
        .collect()
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
    if let Some(content_type) = &request.content_type {
        http_request.push_str(&format!("Content-Type: {content_type}\r\n"));
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

fn parse_single_xml_text(
    action: &str,
    response: &RestResponseObservation,
    element_name: &str,
) -> Result<String, EngineError> {
    ensure_success_xml_response(action, response)?;
    find_xml_text(response_body_text(response)?, element_name).ok_or_else(|| {
        EngineError::AwsResponseParse {
            action: action.to_string(),
            message: format!("missing <{element_name}> in AWS response"),
        }
    })
}

fn parse_sqs_receive_messages(
    action: &str,
    response: &RestResponseObservation,
) -> Result<Vec<SqsMessageObservation>, EngineError> {
    ensure_success_xml_response(action, response)?;
    let body = response_body_text(response)?;
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut messages = Vec::new();
    let mut current_message: Option<SqsMessageBuilder> = None;
    let mut current_attribute_name: Option<String> = None;
    let mut text_target: Option<String> = None;

    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(element)) => {
                let name = xml_name(element.name());
                if name == "Message" {
                    current_message = Some(SqsMessageBuilder::default());
                } else if current_message.is_some() {
                    text_target = Some(name);
                }
            }
            Ok(XmlEvent::Text(text)) => {
                let Some(target) = text_target.as_deref() else {
                    continue;
                };
                let value = text
                    .xml_content()
                    .map_err(|error| EngineError::AwsResponseParse {
                        action: action.to_string(),
                        message: error.to_string(),
                    })?
                    .into_owned();
                if let Some(message) = current_message.as_mut() {
                    match target {
                        "MessageId" => message.message_id = Some(value),
                        "ReceiptHandle" => message.receipt_handle = Some(value),
                        "Body" => message
                            .body
                            .get_or_insert_with(String::new)
                            .push_str(&value),
                        "Name" => current_attribute_name = Some(value),
                        "Value" => {
                            if let Some(name) = current_attribute_name.take() {
                                message.attributes.insert(name, value);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(XmlEvent::GeneralRef(reference)) => {
                let Some(target) = text_target.as_deref() else {
                    continue;
                };
                let value = reference
                    .decode()
                    .map_err(|error| EngineError::AwsResponseParse {
                        action: action.to_string(),
                        message: error.to_string(),
                    })?;
                let value = xml_reference_value(&value);
                if let Some(message) = current_message.as_mut() {
                    match target {
                        "Body" => message
                            .body
                            .get_or_insert_with(String::new)
                            .push_str(&value),
                        "MessageId" => message
                            .message_id
                            .get_or_insert_with(String::new)
                            .push_str(&value),
                        "ReceiptHandle" => message
                            .receipt_handle
                            .get_or_insert_with(String::new)
                            .push_str(&value),
                        "Name" => current_attribute_name
                            .get_or_insert_with(String::new)
                            .push_str(&value),
                        _ => {}
                    }
                }
            }
            Ok(XmlEvent::End(element)) => {
                let name = xml_name(element.name());
                text_target = None;
                if name == "Message" {
                    let message = current_message
                        .take()
                        .ok_or_else(|| EngineError::AwsResponseParse {
                            action: action.to_string(),
                            message: "unexpected </Message>".to_string(),
                        })?
                        .build(action)?;
                    messages.push(message);
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(error) => {
                return Err(EngineError::AwsResponseParse {
                    action: action.to_string(),
                    message: error.to_string(),
                });
            }
            _ => {}
        }
    }

    Ok(messages)
}

#[derive(Default)]
struct SqsMessageBuilder {
    message_id: Option<String>,
    receipt_handle: Option<String>,
    body: Option<String>,
    attributes: BTreeMap<String, String>,
}

impl SqsMessageBuilder {
    fn build(self, action: &str) -> Result<SqsMessageObservation, EngineError> {
        let message_id = self
            .message_id
            .ok_or_else(|| missing_sqs_message_field(action, "MessageId"))?;
        let receipt_handle = self
            .receipt_handle
            .ok_or_else(|| missing_sqs_message_field(action, "ReceiptHandle"))?;
        let body_text = self
            .body
            .ok_or_else(|| missing_sqs_message_field(action, "Body"))?;
        let body = parse_sqs_body(&body_text);
        let body_message_json = body
            .get("Message")
            .and_then(Value::as_str)
            .and_then(|message| serde_json::from_str::<Value>(message).ok());

        Ok(SqsMessageObservation {
            message_id,
            receipt_handle,
            body,
            body_message_json,
            attributes: self.attributes,
        })
    }
}

fn parse_sqs_body(body_text: &str) -> Value {
    serde_json::from_str::<Value>(body_text)
        .or_else(|_| {
            let unescaped = body_text
                .replace("&quot;", "\"")
                .replace("&#34;", "\"")
                .replace("&amp;", "&");
            serde_json::from_str::<Value>(&unescaped)
        })
        .unwrap_or_else(|_| Value::String(body_text.to_string()))
}

fn missing_sqs_message_field(action: &str, field: &str) -> EngineError {
    EngineError::AwsResponseParse {
        action: action.to_string(),
        message: format!("SQS message missing <{field}>"),
    }
}

fn ensure_success_xml_response(
    action: &str,
    response: &RestResponseObservation,
) -> Result<(), EngineError> {
    if !(200..300).contains(&response.status) {
        return Err(EngineError::AwsResponseParse {
            action: action.to_string(),
            message: format!("AWS response returned status {}", response.status),
        });
    }
    Ok(())
}

fn response_body_text(response: &RestResponseObservation) -> Result<&str, EngineError> {
    match &response.body {
        RestBody::Text { value } => Ok(value),
        RestBody::Json { .. } => Err(EngineError::AwsResponseParse {
            action: "AWS".to_string(),
            message: "unexpected JSON AWS response body".to_string(),
        }),
        RestBody::Empty => Err(EngineError::AwsResponseParse {
            action: "AWS".to_string(),
            message: "empty AWS response body".to_string(),
        }),
    }
}

fn find_xml_text(xml: &str, element_name: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut in_target = false;

    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(element)) if xml_name(element.name()) == element_name => {
                in_target = true;
            }
            Ok(XmlEvent::Text(text)) if in_target => {
                return text.xml_content().ok().map(|value| value.into_owned());
            }
            Ok(XmlEvent::End(element)) if xml_name(element.name()) == element_name => {
                in_target = false;
            }
            Ok(XmlEvent::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

fn xml_reference_value(reference: &str) -> String {
    match reference {
        "quot" => "\"".to_string(),
        "apos" => "'".to_string(),
        "amp" => "&".to_string(),
        "lt" => "<".to_string(),
        "gt" => ">".to_string(),
        "#34" | "#x22" | "#X22" => "\"".to_string(),
        _ => format!("&{reference};"),
    }
}

fn sqs_receive_extraction_document(messages: &[SqsMessageObservation]) -> Value {
    let messages = messages
        .iter()
        .map(|message| {
            serde_json::json!({
                "message_id": message.message_id,
                "receipt_handle": message.receipt_handle,
                "body": message.body,
                "body_message_json": message.body_message_json,
                "attributes": message.attributes,
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "messages": messages,
        "message": messages.first().cloned().unwrap_or(Value::Null),
    })
}

fn xml_name(name: quick_xml::name::QName<'_>) -> String {
    String::from_utf8_lossy(name.as_ref()).to_string()
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
