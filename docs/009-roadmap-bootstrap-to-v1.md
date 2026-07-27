# 009 Roadmap: Bootstrap To V1

Status: Draft

Reality will reshape this roadmap. Invariants should remain stable while implementation details change.

## Phase 0: Bootstrap (Current)

- docs
- devcontainer
- project thesis
- invariants
- initial ADRs
- no engine implementation

## Phase 1: Tiny CLI Engine

Goal: prove event loop without real network IO.

- seed event
- handler matching
- emit-event effect
- trace output
- tests for causal relationships

## Phase 2: REST Adapter

- simple GET/POST
- JSON body support
- status assertions
- event emission from response body
- request/response trace entries

## Phase 3: Workflow File Format Draft

- parse workflow files
- parse environment files
- bind environment values
- validate enough to run simple workflows

## Phase 4: GraphQL Adapter

- query/mutation over HTTP
- variables
- parse `data`/`errors`/`extensions`
- GraphQL-aware assertions
- event emission from GraphQL data

Implementation note: a first narrow adapter now exists for local HTTP GraphQL endpoints. It posts query documents with variables, treats GraphQL `errors` as run failures, emits events from `data.*` paths, and is covered by the local `graphql-service` fixture.

## Phase 5: WebSocket Adapter

- connect named session
- send JSON/text
- receive and expectation with timeout
- message-to-event emission
- causal trace integration

Implementation note: a first narrow adapter now exists for local `ws://` endpoints. It opens one connection per effect, sends JSON or text, reads one message with a timeout, checks received payload fields with RFC 9535 JSONPath, emits events from the received message, and is covered by the local `websocket-service` fixture.

## Phase 6: SQS Adapter

- send
- poll until match
- correlation using run id
- delete policy
- message-to-event emission
- causal trace integration

Implementation note: a first narrow GoAWS-backed adapter now exists for SNS publish, SQS send, and SQS receive/delete-on-success. It parses GoAWS XML query responses, exposes received SQS message bodies and SNS notification payload JSON to JSONPath extraction, and is covered by `sns-sqs-smoke`.

## Phase 7: Safety And Run Planning

- declared capabilities
- risk levels
- run plan display
- secret references and masking
- dry-run where possible

Implementation note: advisory run planning now exists in core and CLI. It reports effect order and required capabilities for local workflow actions, HTTP/GraphQL, SNS/SQS, and WebSocket effects. Capability enforcement remains future work.

## Phase 8: Desktop UI Exploration

Stack: Tauri + Vue + shadcn-vue.

- UI consumes engine API/core
- project browser
- workflow editor
- trace viewer
- run console

Implementation note: an initial desktop shell now exists in `apps/desktop`. It lists example workflows, requests plans from `devknife-core`, calls the engine through Tauri commands, renders the returned trace/report in Vue, and provides a repository-confined YAML editor with validation and guarded saving.

## Exit Criteria For V1 Candidate

- event-native core proven in real protocol flows
- first-class support for REST/GraphQL/SQS/WebSocket in declared scope
- file-backed artifacts with stable draft schema
- causal trace sufficient for failure diagnosis
- Linux and Windows validation path

## Phase 1 Implementation Note

The first implementation pass started Phase 1 with a synchronous in-memory engine, typed causal trace, YAML bootstrap workflow loading, and CLI `run`/`validate` commands. The local protocol testbed was also added early so REST, GraphQL, SQS, and WebSocket adapter work had deterministic fixtures.

Several real protocol effects now exist. Phase 3 should continue focusing on schema hardening, environment binding, diagnostics, and trace artifact persistence rather than first YAML parsing.
