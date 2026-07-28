# 011 Production-Shaped Demo

Status: Active V1 acceptance story

The V1 demo is a deterministic customer onboarding workflow that runs against local services while
preserving a production-shaped causal story.

## Scenario

A customer onboarding request should:

1. create the customer account through REST,
2. invite the primary operator through REST using the returned account ID,
3. project the workspace read model through GraphQL,
4. subscribe to live onboarding status over WebSocket,
5. publish the onboarding event to SNS,
6. consume the event from SQS,
7. assert that the status is ready for the operator, and
8. write a final acceptance message to the results queue.

The workflow is intentionally not a request list. Each observation emits the next domain event, and
the trace explains why the following protocol operation happened.

## Artifacts

- Workflow: `examples/workflows/customer-onboarding-demo.workflow.yaml`
- Environment: `examples/environments/local.yaml`
- Runner: `testbed/bin/customer-onboarding-demo`

## Run

```sh
testbed/bin/customer-onboarding-demo
```

The runner starts the REST, GraphQL, WebSocket, and GoAWS fixtures, waits for health checks, runs
the workflow with write capabilities approved, and tears the fixture stack down afterward.

Equivalent manual run when the fixture stack is already up:

```sh
cargo run -p devknife-cli -- run examples/workflows/customer-onboarding-demo.workflow.yaml --environment examples/environments/local.yaml --allow-write
```

## Acceptance Signal

A passing run should end with a record effect whose message is:

```text
customer onboarding completed across REST, GraphQL, WebSocket, SNS, and SQS
```

The persisted trace under `runs/` is the primary demo artifact. It should show the chain from
`customer.onboarding.requested` through account creation, operator invitation, workspace projection,
live status subscription, event publication, queue consumption, assertions, and final acceptance
recording.

## Why This Is The V1 Demo

This is still deterministic and local, but it demonstrates the core product claim with recognizable
service boundaries:

- REST models command-style service calls.
- GraphQL models read-model projection.
- WebSocket models live status subscription.
- SNS/SQS model asynchronous integration.
- The trace ties them together as cause and effect.

The demo remains narrow by design. It does not require hosted infrastructure, real cloud
credentials, imported schemas, external auth, retries, or plugin support.
