use devknife_core::{
    Effect, Event, ExecutionLimits, Handler, Observation, RestAssertionObservation, RunStatus,
    Runner, RuntimeEnvironment, ServiceBinding, TraceEntryKind, Workflow,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    thread,
};

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

#[test]
fn rest_effect_builds_request_from_event_and_environment() {
    let server = RestTestServer::start(|request| {
        assert!(request.starts_with("POST /accounts?source=test HTTP/1.1"));
        assert!(request.contains("x-correlation-id: corr-001"));
        assert!(request.contains(r#"{"name":"Acme"}"#));
        http_response(201, r#"{"id":"acct_created_001"}"#)
    });
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: rest-builds-request
seed_events:
  - id: seed-create
    type: account.create.requested
    payload:
      name: Acme
      correlation_id: corr-001
handlers:
  - on: account.create.requested
    effects:
      - type: rest
        service: rest
        operation: create_account
        method: POST
        path: /accounts
        query:
          source: test
        headers:
          x-correlation-id: "{{ event.payload.correlation_id }}"
        json_body:
          name: "{{ event.payload.name }}"
        expect:
          status: 201
"#,
    )
    .expect("workflow parses");

    let report = runner_with_rest(server.base_url()).run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    assert!(report.trace.iter().any(|entry| {
        matches!(
            &entry.kind,
            TraceEntryKind::EffectExecuted {
                observation:
                    Observation::RestResponse {
                        response,
                        assertions,
                        ..
                    },
                ..
            } if response.status == 201
                && matches!(
                    assertions.as_slice(),
                    [RestAssertionObservation::StatusPassed { expected: 201, actual: 201 }]
                )
        )
    }));
}

#[test]
fn successful_rest_response_emits_event_from_json_body() {
    let server =
        RestTestServer::start(|_| http_response(200, r#"{"id":"acct_001","name":"Demo"}"#));
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: rest-emits
seed_events:
  - id: seed-load
    type: account.load.requested
    payload:
      account_id: acct_001
handlers:
  - on: account.load.requested
    effects:
      - type: rest
        service: rest
        operation: get_account
        method: GET
        path: /accounts/{{ event.payload.account_id }}
        expect:
          status: 200
        emits:
          - event_type: account.loaded
            payload:
              account_id: body.id
              name:
                from: body.name
"#,
    )
    .expect("workflow parses");

    let report = runner_with_rest(server.base_url()).run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    let emitted = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation: Observation::RestResponse { emitted_events, .. },
                ..
            } => emitted_events.first(),
            _ => None,
        })
        .expect("REST emitted event");
    assert_eq!(emitted.event_type, "account.loaded");
    assert_eq!(
        emitted.payload,
        json!({"account_id": "acct_001", "name": "Demo"})
    );
}

#[test]
fn failed_rest_status_assertion_fails_run() {
    let server = RestTestServer::start(|_| http_response(404, r#"{"error":"missing"}"#));
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: rest-fails-status
seed_events:
  - id: seed-load
    type: account.load.requested
handlers:
  - on: account.load.requested
    effects:
      - type: rest
        service: rest
        operation: get_account
        method: GET
        path: /accounts/missing
        expect:
          status: 200
"#,
    )
    .expect("workflow parses");

    let report = runner_with_rest(server.base_url()).run(workflow);

    assert_eq!(report.status, RunStatus::Failed);
    let failure = report.failure.expect("failure");
    assert!(failure
        .message
        .contains("event seed-load triggered handler 0 effect 0"));
    assert!(failure.message.contains("status expected 200, actual 404"));
    assert!(report.trace.iter().any(|entry| {
        matches!(
            &entry.kind,
            TraceEntryKind::EffectExecuted {
                observation:
                    Observation::RestResponse {
                        response,
                        assertions,
                        ..
                    },
                ..
            } if response.status == 404
                && assertions.iter().any(|assertion| matches!(
                    assertion,
                    RestAssertionObservation::StatusFailed {
                        expected: 200,
                        actual: 404
                    }
                ))
        )
    }));
}

#[test]
fn missing_rest_service_binding_fails_run_clearly() {
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: missing-rest-binding
seed_events:
  - id: seed-load
    type: account.load.requested
handlers:
  - on: account.load.requested
    effects:
      - type: rest
        service: rest
        method: GET
        path: /accounts/acct_001
"#,
    )
    .expect("workflow parses");

    let report = Runner::default().run(workflow);

    assert_eq!(report.status, RunStatus::Failed);
    assert!(report
        .failure
        .expect("failure")
        .message
        .contains("missing REST service binding for 'rest'"));
    assert!(report.trace.iter().any(|entry| {
        matches!(
            &entry.kind,
            TraceEntryKind::EffectExecuted {
                event_id,
                handler_index: 0,
                effect_index: 0,
                observation: Observation::RestFailed { message, .. },
                ..
            } if event_id == "seed-load" && message.contains("missing REST service binding")
        )
    }));
}

#[test]
fn trace_preserves_causality_through_rest_effect() {
    let server = RestTestServer::start(|_| http_response(200, r#"{"id":"acct_001"}"#));
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: rest-causality
seed_events:
  - id: seed-load
    type: account.load.requested
handlers:
  - on: account.load.requested
    effects:
      - type: rest
        service: rest
        operation: get_account
        method: GET
        path: /accounts/acct_001
        expect:
          status: 200
        emits:
          - event_type: account.loaded
            payload:
              account_id: body.id
"#,
    )
    .expect("workflow parses");

    let report = runner_with_rest(server.base_url()).run(workflow);
    let (effect_trace_id, emitted) = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                event_id,
                handler_index,
                effect_index,
                observation:
                    Observation::RestResponse {
                        operation,
                        response,
                        emitted_events,
                        ..
                    },
                ..
            } => {
                assert_eq!(event_id, "seed-load");
                assert_eq!(*handler_index, 0);
                assert_eq!(*effect_index, 0);
                assert_eq!(operation.method, "GET");
                assert_eq!(response.status, 200);
                Some((entry.id.clone(), emitted_events[0].clone()))
            }
            _ => None,
        })
        .expect("REST trace entry");

    let cause = emitted.caused_by.expect("cause");
    assert_eq!(cause.event_id, "seed-load");
    assert_eq!(cause.trace_entry_id, effect_trace_id);
    assert!(report.trace.iter().any(|entry| {
        matches!(
            &entry.kind,
            TraceEntryKind::EventDequeued { event }
                if event.event_type == "account.loaded"
        )
    }));
}

fn runner_with_rest(base_url: String) -> Runner {
    let mut services = BTreeMap::new();
    services.insert("rest".to_string(), ServiceBinding { base_url });
    Runner::with_environment(
        ExecutionLimits::default(),
        RuntimeEnvironment {
            services,
            ..RuntimeEnvironment::default()
        },
    )
}

struct RestTestServer {
    base_url: String,
}

impl RestTestServer {
    fn start(handler: impl FnOnce(String) -> String + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let port = listener.local_addr().expect("server address").port();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0; 4096];
            let size = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..size]).to_string();
            let response = handler(request);
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        Self {
            base_url: format!("http://127.0.0.1:{port}"),
        }
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }
}

fn http_response(status: u16, body: &str) -> String {
    let reason = match status {
        200 => "OK",
        201 => "Created",
        404 => "Not Found",
        _ => "Status",
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    )
}
