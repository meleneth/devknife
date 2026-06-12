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

## Phase 5: WebSocket Adapter

- connect named session
- send JSON/text
- receive and expectation with timeout
- message-to-event emission
- causal trace integration

## Phase 6: SQS Adapter

- send
- poll until match
- correlation using run id
- delete policy
- message-to-event emission
- causal trace integration

## Phase 7: Safety And Run Planning

- declared capabilities
- risk levels
- run plan display
- secret references and masking
- dry-run where possible

## Phase 8: Desktop UI Exploration

Likely stack: Tauri + Vue.

- UI consumes engine API/core
- project browser
- workflow editor
- trace viewer
- run console

## Exit Criteria For V1 Candidate

- event-native core proven in real protocol flows
- first-class support for REST/GraphQL/SQS/WebSocket in declared scope
- file-backed artifacts with stable draft schema
- causal trace sufficient for failure diagnosis
- Linux and Windows validation path

## Phase 1 Implementation Note

The first implementation pass has started Phase 1 with a synchronous in-memory engine, typed causal trace, YAML bootstrap workflow loading, and CLI `run`/`validate` commands. The local protocol testbed was also added early so REST, GraphQL, SQS, and WebSocket adapter work has deterministic fixtures when those phases begin.

Real protocol effects remain deferred to later phases. Phase 3 should now focus on schema hardening, environment binding, diagnostics, and trace artifact persistence rather than first YAML parsing.
