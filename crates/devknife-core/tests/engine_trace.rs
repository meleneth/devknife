use devknife_core::{
    Effect, Event, ExecutionLimits, Handler, Observation, RunStatus, Runner, TraceEntryKind,
    Workflow,
};
use serde_json::json;

#[test]
fn seed_event_with_no_handlers_traces_and_succeeds() {
    let workflow = Workflow {
        name: "no-handlers".to_string(),
        seed_events: vec![Event::seed("seed-1", "workflow.started", json!({}))],
        handlers: vec![],
    };

    let report = Runner::default().run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    assert!(report
        .trace
        .iter()
        .any(|entry| matches!(entry.kind, TraceEntryKind::EventSeeded { .. })));
    assert!(report
        .trace
        .iter()
        .any(|entry| matches!(entry.kind, TraceEntryKind::HandlerSkipped { .. })));
}

#[test]
fn emit_effect_enqueues_emitted_event() {
    let workflow = Workflow {
        name: "emit".to_string(),
        seed_events: vec![Event::seed("seed-1", "workflow.started", json!({}))],
        handlers: vec![
            Handler {
                on: "workflow.started".to_string(),
                effects: vec![Effect::Emit {
                    event_type: "greeting.created".to_string(),
                    payload: json!({ "message": "hello" }),
                }],
            },
            Handler {
                on: "greeting.created".to_string(),
                effects: vec![Effect::Record {
                    message: "observed greeting".to_string(),
                }],
            },
        ],
    };

    let report = Runner::default().run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    assert!(report.trace.iter().any(|entry| {
        matches!(
            &entry.kind,
            TraceEntryKind::EventDequeued { event }
                if event.event_type == "greeting.created"
        )
    }));
}

#[test]
fn emitted_event_links_back_to_effect_that_created_it() {
    let workflow = Workflow {
        name: "causal".to_string(),
        seed_events: vec![Event::seed("seed-1", "workflow.started", json!({}))],
        handlers: vec![Handler {
            on: "workflow.started".to_string(),
            effects: vec![Effect::Emit {
                event_type: "next".to_string(),
                payload: json!({}),
            }],
        }],
    };

    let report = Runner::default().run(workflow);
    let (effect_trace_id, emitted) = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation: Observation::EmittedEvents { events },
                ..
            } => Some((entry.id.clone(), events[0].clone())),
            _ => None,
        })
        .expect("emit effect trace entry");

    let cause = emitted.caused_by.expect("emitted event cause");
    assert_eq!(cause.event_id, "seed-1");
    assert_eq!(cause.trace_entry_id, effect_trace_id);
}

#[test]
fn multiple_handlers_for_same_event_are_deterministic() {
    let workflow = Workflow {
        name: "deterministic".to_string(),
        seed_events: vec![Event::seed("seed-1", "workflow.started", json!({}))],
        handlers: vec![
            Handler {
                on: "workflow.started".to_string(),
                effects: vec![Effect::Record {
                    message: "first".to_string(),
                }],
            },
            Handler {
                on: "workflow.started".to_string(),
                effects: vec![Effect::Record {
                    message: "second".to_string(),
                }],
            },
        ],
    };

    let report = Runner::default().run(workflow);
    let messages: Vec<_> = report
        .trace
        .iter()
        .filter_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation: Observation::RecordedMessage { message },
                ..
            } => Some(message.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(messages, vec!["first", "second"]);
}

#[test]
fn max_event_count_prevents_infinite_loops() {
    let workflow = Workflow {
        name: "loop".to_string(),
        seed_events: vec![Event::seed("seed-1", "tick", json!({}))],
        handlers: vec![Handler {
            on: "tick".to_string(),
            effects: vec![Effect::Emit {
                event_type: "tick".to_string(),
                payload: json!({}),
            }],
        }],
    };

    let runner = Runner::new(ExecutionLimits {
        max_events: 3,
        max_steps: 20,
        max_depth: 20,
    });
    let report = runner.run(workflow);

    assert_eq!(report.status, RunStatus::Failed);
    assert!(report
        .failure
        .expect("failure")
        .message
        .contains("max event count exceeded"));
}

#[test]
fn failed_assertion_fails_run_and_records_failure() {
    let workflow = Workflow {
        name: "assertion".to_string(),
        seed_events: vec![Event::seed("seed-1", "check", json!({ "ok": false }))],
        handlers: vec![Handler {
            on: "check".to_string(),
            effects: vec![Effect::Assert(devknife_core::domain::AssertEffect {
                path: "ok".to_string(),
                equals: json!(true),
            })],
        }],
    };

    let report = Runner::default().run(workflow);

    assert_eq!(report.status, RunStatus::Failed);
    assert!(report.trace.iter().any(|entry| {
        matches!(
            &entry.kind,
            TraceEntryKind::EffectExecuted {
                observation: Observation::AssertionFailed { path, .. },
                ..
            } if path == "ok"
        )
    }));
}

#[test]
fn event_payload_is_flexible_but_event_envelope_is_typed() {
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: flexible-payload
seed_events:
  - type: payload.seen
    payload:
      nested:
        count: 2
handlers:
  - on: payload.seen
    effects:
      - type: assert
        path: nested.count
        equals: 2
"#,
    )
    .expect("workflow parses");

    assert_eq!(workflow.seed_events[0].event_type, "payload.seen");
    assert_eq!(workflow.seed_events[0].payload["nested"]["count"], json!(2));
    assert_eq!(Runner::default().run(workflow).status, RunStatus::Succeeded);
}
