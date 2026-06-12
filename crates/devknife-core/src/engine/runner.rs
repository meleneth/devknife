use std::collections::VecDeque;

use serde_json::Value;
use uuid::Uuid;

use crate::domain::{
    AssertEffect, Effect, Event, EventCause, Observation, RunReport, RunStatus, TraceEntry,
    TraceEntryKind, TraceFailure, Workflow,
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
}

impl Runner {
    pub fn new(limits: ExecutionLimits) -> Self {
        Self { limits }
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

                    if let Observation::AssertionFailed { .. } = observation {
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
                        return state.fail(
                            Some(trace_entry_id),
                            "assertion failed during workflow run".to_string(),
                        );
                    }

                    let emitted = match &observation {
                        Observation::EmittedEvents { events } => events.clone(),
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
        }
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
