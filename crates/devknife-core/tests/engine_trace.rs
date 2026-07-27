use devknife_core::{
    default_workflow_version, plan_workflow, CapabilityRisk, Effect, Event, ExecutionLimits,
    GraphqlAssertionObservation, Handler, Observation, RestAssertionObservation, RunStatus, Runner,
    RuntimeEnvironment, ServiceBinding, TraceEntryKind, Workflow, CURRENT_WORKFLOW_VERSION,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    thread,
};
use tungstenite::{accept, Message as WsMessage};

#[test]
fn seed_event_with_no_handlers_traces_and_succeeds() {
    let workflow = Workflow {
        version: default_workflow_version(),
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
        version: default_workflow_version(),
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
        version: default_workflow_version(),
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
        version: default_workflow_version(),
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
        version: default_workflow_version(),
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
        version: default_workflow_version(),
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
fn workflow_version_defaults_to_current_and_rejects_unknown_versions() {
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: default-version
seed_events:
  - type: workflow.started
"#,
    )
    .expect("workflow parses");
    assert_eq!(workflow.version, CURRENT_WORKFLOW_VERSION);

    let unsupported = devknife_core::load_workflow_yaml(
        r#"
version: devknife.workflow/v9
name: unsupported-version
seed_events:
  - type: workflow.started
"#,
    );
    assert!(unsupported.is_err(), "unsupported versions must fail");
}

#[test]
fn workflow_plan_lists_required_capabilities_and_effect_order() {
    let workflow = devknife_core::load_workflow_yaml(
        r#"
version: devknife.workflow/v1alpha1
name: planned
seed_events:
  - type: workflow.started
handlers:
  - on: workflow.started
    effects:
      - type: rest
        base_url: http://localhost:18101
        method: GET
        path: /health
      - type: sns_publish
        endpoint_url: http://localhost:18104
        topic_arn: arn:aws:sns:us-east-1:100010001000:devknife-events
        message:
          ok: true
      - type: sqs_receive
        endpoint_url: http://localhost:18104
        queue_url: http://localhost:18104/100010001000/devknife-workflow-input
        delete_on_success: true
"#,
    )
    .expect("workflow parses");

    let plan = plan_workflow(&workflow);

    assert_eq!(plan.workflow_name, "planned");
    assert_eq!(plan.workflow_version, CURRENT_WORKFLOW_VERSION);
    assert_eq!(
        plan.effects
            .iter()
            .map(|effect| effect.effect_type.as_str())
            .collect::<Vec<_>>(),
        vec!["rest", "sns_publish", "sqs_receive"]
    );
    let capability_ids = plan
        .required_capabilities
        .iter()
        .map(|capability| capability.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        capability_ids,
        vec![
            "aws.sns.publish",
            "aws.sqs.delete",
            "aws.sqs.receive",
            "network.http.read"
        ]
    );
    assert!(plan
        .required_capabilities
        .iter()
        .any(|capability| capability.id == "aws.sqs.delete"
            && capability.risk == CapabilityRisk::Write));
}

#[test]
fn execution_policy_denies_write_capabilities_before_effects_run() {
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: denied-write
seed_events:
  - id: seed
    type: account.create.requested
handlers:
  - on: account.create.requested
    effects:
      - type: rest
        operation: create_account
        method: POST
        base_url: http://127.0.0.1:1
        path: /accounts
"#,
    )
    .expect("workflow parses");

    let report = Runner::with_environment_and_policy(
        ExecutionLimits::default(),
        devknife_core::RuntimeEnvironment::default(),
        devknife_core::ExecutionPolicy::deny_write(),
    )
    .run(workflow);

    assert_eq!(report.status, RunStatus::Failed);
    assert!(report
        .failure
        .expect("failure")
        .message
        .contains("network.http.write"));
    assert_eq!(report.trace.len(), 2);
}

#[test]
fn response_emission_requires_jsonpath_from_selector() {
    let shorthand = devknife_core::load_workflow_yaml(
        r#"
name: rejects-shorthand
seed_events:
  - type: account.load.requested
handlers:
  - on: account.load.requested
    effects:
      - type: rest
        base_url: http://localhost:18101
        method: GET
        path: /accounts/acct_001
        emits:
          - event_type: account.loaded
            payload:
              account_id: body.id
"#,
    );
    assert!(
        shorthand.is_err(),
        "legacy dot-path shorthand must not be accepted"
    );

    let invalid_jsonpath = devknife_core::load_workflow_yaml(
        r#"
name: rejects-invalid-jsonpath
seed_events:
  - type: account.load.requested
handlers:
  - on: account.load.requested
    effects:
      - type: rest
        base_url: http://localhost:18101
        method: GET
        path: /accounts/acct_001
        emits:
          - event_type: account.loaded
            payload:
              account_id:
                from: body.id
"#,
    );
    assert!(
        invalid_jsonpath.is_err(),
        "selectors must be RFC 9535 JSONPath"
    );
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
              account_id:
                from: $.body.id
              name:
                from: $.body.name
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
              account_id:
                from: $.body.id
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

#[test]
fn graphql_effect_posts_query_and_emits_event_from_data() {
    let server = RestTestServer::start(|request| {
        assert!(request.starts_with("POST /graphql HTTP/1.1"));
        assert!(request.contains(r#""operationName":"AccountUsers""#));
        assert!(request.contains(r#""id":"acct_001""#));
        http_response(
            200,
            r#"{"data":{"account":{"id":"acct_001","name":"Demo","users":[{"id":"user_001","email":"ava@example.test"}]}}}"#,
        )
    });
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: graphql-emits
seed_events:
  - id: seed-load
    type: account.users.load.requested
    payload:
      account_id: acct_001
handlers:
  - on: account.users.load.requested
    effects:
      - type: graphql
        service: graphql
        operation_name: AccountUsers
        query: |
          query AccountUsers($id: ID!) {
            account(id: $id) {
              id
              name
              users { id email }
            }
          }
        variables:
          id: "{{ event.payload.account_id }}"
        expect:
          status: 200
        emits:
          - event_type: account.users.loaded
            payload:
              account_id:
                from: $.data.account.id
              first_user_email:
                from: $.data.account.users[0].email
              users:
                from: $.data.account.users
"#,
    )
    .expect("workflow parses");

    let report = runner_with_graphql(server.base_url_with_path("/graphql")).run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    let emitted = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation:
                    Observation::GraphqlResponse {
                        assertions,
                        emitted_events,
                        ..
                    },
                ..
            } => {
                assert!(assertions.iter().any(|assertion| matches!(
                    assertion,
                    GraphqlAssertionObservation::NoErrorsPassed
                )));
                emitted_events.first()
            }
            _ => None,
        })
        .expect("GraphQL emitted event");
    assert_eq!(emitted.event_type, "account.users.loaded");
    assert_eq!(emitted.payload["account_id"], json!("acct_001"));
    assert_eq!(
        emitted.payload["first_user_email"],
        json!("ava@example.test")
    );
    assert_eq!(
        emitted.payload["users"][0]["email"],
        json!("ava@example.test")
    );
}

#[test]
fn graphql_errors_fail_run_even_with_http_200() {
    let server = RestTestServer::start(|_| {
        http_response(
            200,
            r#"{"data":{"account":null},"errors":[{"message":"account failed"}]}"#,
        )
    });
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: graphql-errors
seed_events:
  - id: seed-load
    type: account.users.load.requested
handlers:
  - on: account.users.load.requested
    effects:
      - type: graphql
        service: graphql
        operation_name: AccountUsers
        query: |
          query AccountUsers($id: ID!) {
            account(id: $id) { id }
          }
        variables:
          id: acct_001
        expect:
          status: 200
"#,
    )
    .expect("workflow parses");

    let report = runner_with_graphql(server.base_url_with_path("/graphql")).run(workflow);

    assert_eq!(report.status, RunStatus::Failed);
    assert!(report
        .failure
        .expect("failure")
        .message
        .contains("returned 1 error(s)"));
    assert!(report.trace.iter().any(|entry| {
        matches!(
            &entry.kind,
            TraceEntryKind::EffectExecuted {
                observation:
                    Observation::GraphqlResponse {
                        assertions,
                        ..
                    },
                ..
            } if assertions.iter().any(|assertion| matches!(
                assertion,
                GraphqlAssertionObservation::NoErrorsFailed { errors } if errors.len() == 1
            ))
        )
    }));
}

#[test]
fn sns_publish_emits_message_id() {
    let server = AwsTestServer::start(1, |request_index, request| {
        assert_eq!(request_index, 0);
        assert!(request.starts_with("POST / HTTP/1.1"));
        assert!(request.contains("Action=Publish"));
        assert!(request
            .contains("TopicArn=arn%3Aaws%3Asns%3Aus-east-1%3A100010001000%3Adevknife-events"));
        assert!(request.contains("Message=%7B%22correlation_id%22%3A%22corr-001%22%7D"));
        http_response(
            200,
            r#"<PublishResponse><PublishResult><MessageId>sns-message-001</MessageId></PublishResult></PublishResponse>"#,
        )
    });
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: sns-publish
seed_events:
  - id: seed-publish
    type: workflow.event.ready
    payload:
      correlation_id: corr-001
handlers:
  - on: workflow.event.ready
    effects:
      - type: sns_publish
        service: aws
        topic_arn: arn:aws:sns:us-east-1:100010001000:devknife-events
        message:
          correlation_id: "{{ event.payload.correlation_id }}"
        emits:
          - event_type: sns.published
            payload:
              message_id:
                from: $.message_id
"#,
    )
    .expect("workflow parses");

    let report = runner_with_aws(server.base_url()).run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    let emitted = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation: Observation::SnsPublish { emitted_events, .. },
                ..
            } => emitted_events.first(),
            _ => None,
        })
        .expect("SNS emitted event");
    assert_eq!(emitted.event_type, "sns.published");
    assert_eq!(emitted.payload["message_id"], json!("sns-message-001"));
}

#[test]
fn sqs_send_posts_message_and_emits_message_id() {
    let server = AwsTestServer::start(1, |request_index, request| {
        assert_eq!(request_index, 0);
        assert!(request.starts_with("POST /100010001000/devknife-workflow-results HTTP/1.1"));
        assert!(request.contains("Action=SendMessage"));
        assert!(request.contains("MessageBody=%7B%22result%22%3A%22ok%22%7D"));
        http_response(
            200,
            r#"<SendMessageResponse><SendMessageResult><MessageId>sqs-message-001</MessageId></SendMessageResult></SendMessageResponse>"#,
        )
    });
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: sqs-send
seed_events:
  - id: seed-send
    type: workflow.result.ready
handlers:
  - on: workflow.result.ready
    effects:
      - type: sqs_send
        service: aws
        queue_url: http://localhost:18104/100010001000/devknife-workflow-results
        message:
          result: ok
        emits:
          - event_type: sqs.sent
            payload:
              message_id:
                from: $.message_id
"#,
    )
    .expect("workflow parses");

    let report = runner_with_aws(server.base_url()).run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    let emitted = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation: Observation::SqsSend { emitted_events, .. },
                ..
            } => emitted_events.first(),
            _ => None,
        })
        .expect("SQS send emitted event");
    assert_eq!(emitted.event_type, "sqs.sent");
    assert_eq!(emitted.payload["message_id"], json!("sqs-message-001"));
}

#[test]
fn sqs_receive_emits_from_sns_message_json_and_deletes() {
    let server = AwsTestServer::start(2, |request_index, request| match request_index {
        0 => {
            assert!(request.starts_with("POST /100010001000/devknife-workflow-input HTTP/1.1"));
            assert!(request.contains("Action=ReceiveMessage"));
            http_response(
                200,
                r#"<ReceiveMessageResponse><ReceiveMessageResult><Message><MessageId>sqs-message-001</MessageId><ReceiptHandle>receipt-001</ReceiptHandle><Body>{&quot;Type&quot;:&quot;Notification&quot;,&quot;Message&quot;:&quot;{\&quot;correlation_id\&quot;:\&quot;corr-001\&quot;,\&quot;status\&quot;:\&quot;ok\&quot;}&quot;}</Body><Attribute><Name>ApproximateReceiveCount</Name><Value>1</Value></Attribute></Message></ReceiveMessageResult></ReceiveMessageResponse>"#,
            )
        }
        1 => {
            assert!(request.starts_with("POST /100010001000/devknife-workflow-input HTTP/1.1"));
            assert!(request.contains("Action=DeleteMessage"));
            assert!(request.contains("ReceiptHandle=receipt-001"));
            http_response(
                200,
                r#"<DeleteMessageResponse><ResponseMetadata><RequestId>delete-001</RequestId></ResponseMetadata></DeleteMessageResponse>"#,
            )
        }
        _ => unreachable!("unexpected request"),
    });
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: sqs-receive
seed_events:
  - id: seed-receive
    type: workflow.result.awaited
handlers:
  - on: workflow.result.awaited
    effects:
      - type: sqs_receive
        service: aws
        queue_url: http://localhost:18104/100010001000/devknife-workflow-input
        wait_time_seconds: 1
        delete_on_success: true
        emits:
          - event_type: workflow.result.received
            payload:
              correlation_id:
                from: $.message.body_message_json.correlation_id
              status:
                from: $.message.body_message_json.status
              receive_count:
                from: $.message.attributes.ApproximateReceiveCount
"#,
    )
    .expect("workflow parses");

    let report = runner_with_aws(server.base_url()).run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    let emitted = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation:
                    Observation::SqsReceive {
                        deleted_receipt_handles,
                        emitted_events,
                        ..
                    },
                ..
            } => {
                assert_eq!(deleted_receipt_handles, &vec!["receipt-001".to_string()]);
                emitted_events.first()
            }
            _ => None,
        })
        .expect("SQS receive emitted event");
    assert_eq!(emitted.event_type, "workflow.result.received");
    assert_eq!(emitted.payload["correlation_id"], json!("corr-001"));
    assert_eq!(emitted.payload["status"], json!("ok"));
    assert_eq!(emitted.payload["receive_count"], json!("1"));
}

#[test]
fn websocket_effect_sends_json_expects_message_and_emits_event() {
    let server = WebsocketTestServer::start(|message| {
        let text = message.into_text().expect("text websocket message");
        assert!(text.contains(r#""type":"ping""#));
        assert!(text.contains(r#""correlation_id":"ws-001""#));
        WsMessage::text(r#"{"type":"pong","correlation_id":"ws-001"}"#)
    });
    let workflow = devknife_core::load_workflow_yaml(
        r#"
name: websocket-ping
seed_events:
  - id: seed-ping
    type: websocket.ping.requested
    payload:
      correlation_id: ws-001
handlers:
  - on: websocket.ping.requested
    effects:
      - type: websocket
        service: websocket
        session: demo
        send:
          json:
            type: ping
            correlation_id: "{{ event.payload.correlation_id }}"
        expect:
          json:
            "$.type": pong
            "$.correlation_id": ws-001
        emits:
          - event_type: websocket.pong.received
            payload:
              message_type:
                from: $.type
              correlation_id:
                from: $.correlation_id
"#,
    )
    .expect("workflow parses");

    let report = runner_with_websocket(server.url()).run(workflow);

    assert_eq!(report.status, RunStatus::Succeeded);
    let emitted = report
        .trace
        .iter()
        .find_map(|entry| match &entry.kind {
            TraceEntryKind::EffectExecuted {
                observation: Observation::WebsocketMessage { emitted_events, .. },
                ..
            } => emitted_events.first(),
            _ => None,
        })
        .expect("WebSocket emitted event");
    assert_eq!(emitted.event_type, "websocket.pong.received");
    assert_eq!(emitted.payload["message_type"], json!("pong"));
    assert_eq!(emitted.payload["correlation_id"], json!("ws-001"));
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

fn runner_with_graphql(base_url: String) -> Runner {
    let mut services = BTreeMap::new();
    services.insert("graphql".to_string(), ServiceBinding { base_url });
    Runner::with_environment(
        ExecutionLimits::default(),
        RuntimeEnvironment {
            services,
            ..RuntimeEnvironment::default()
        },
    )
}

fn runner_with_aws(base_url: String) -> Runner {
    let mut services = BTreeMap::new();
    services.insert("aws".to_string(), ServiceBinding { base_url });
    Runner::with_environment(
        ExecutionLimits::default(),
        RuntimeEnvironment {
            services,
            ..RuntimeEnvironment::default()
        },
    )
}

fn runner_with_websocket(base_url: String) -> Runner {
    let mut services = BTreeMap::new();
    services.insert("websocket".to_string(), ServiceBinding { base_url });
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

    fn base_url_with_path(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }
}

struct AwsTestServer {
    base_url: String,
}

impl AwsTestServer {
    fn start(
        request_count: usize,
        mut handler: impl FnMut(usize, String) -> String + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let port = listener.local_addr().expect("server address").port();
        thread::spawn(move || {
            for request_index in 0..request_count {
                let (mut stream, _) = listener.accept().expect("accept request");
                let mut buffer = [0; 4096];
                let size = stream.read(&mut buffer).expect("read request");
                let request = String::from_utf8_lossy(&buffer[..size]).to_string();
                let response = handler(request_index, request);
                stream
                    .write_all(response.as_bytes())
                    .expect("write response");
            }
        });

        Self {
            base_url: format!("http://127.0.0.1:{port}"),
        }
    }

    fn base_url(&self) -> String {
        self.base_url.clone()
    }
}

struct WebsocketTestServer {
    url: String,
}

impl WebsocketTestServer {
    fn start(handler: impl FnOnce(WsMessage) -> WsMessage + Send + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind websocket test server");
        let port = listener.local_addr().expect("server address").port();
        thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept websocket request");
            let mut socket = accept(stream).expect("websocket handshake");
            let request = socket.read().expect("read websocket message");
            let response = handler(request);
            socket.send(response).expect("write websocket response");
            let _ = socket.close(None);
        });

        Self {
            url: format!("ws://127.0.0.1:{port}/ws"),
        }
    }

    fn url(&self) -> String {
        self.url.clone()
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
