# 009 Roadmap: Bootstrap To V1

Status: Active — V1 candidate preparation

Reality has reshaped this roadmap. The original bootstrap phases are retained as a record of the
implementation sequence, while the current milestone and remaining work are stated explicitly.
Invariants remain the architectural constraints when priorities change.

## Current Position

Phases 0 through 8 are implemented in their declared narrow scope. Devknife now has:

- a Rust workspace with an event-native engine and CLI
- versioned YAML workflow and environment artifacts
- REST, GraphQL, WebSocket, SNS, and SQS effects
- deterministic local fixtures and cross-protocol smoke workflows
- causal trace generation and bounded, confined trace persistence
- capability planning, exact per-capability approvals, dry runs, and secret redaction
- a Tauri/Vue workflow browser, editor, planner, runner, and trace inspector
- Linux and Windows CI for formatting, Clippy, Rust tests, and the desktop production build

The project is no longer in bootstrap. The active milestone is to turn this working vertical slice
into a deliberately scoped V1 candidate.

## Completed Phases

### Phase 0: Bootstrap — Complete

- product thesis, invariants, domain model, ADRs, and devcontainer/tooling baseline

### Phase 1: Tiny CLI Engine — Complete

- deterministic seed-event loop and handler matching
- in-memory emit, record, and assertion effects
- typed causal trace output and relationship tests

### Phase 2: REST Adapter — Complete In Declared Scope

- GET and POST with headers, query parameters, and JSON bodies
- status assertions, response observations, and JSONPath event emission
- chained fixture workflow that passes a created resource ID into a subsequent API call

### Phase 3: Workflow File Format Draft — Complete In `v1alpha1`

- strict `devknife.workflow/v1alpha1` YAML loading
- environment loading, template validation, and binding preflight
- unknown-field rejection, protocol setting validation, and actionable diagnostics
- `.yaml` and `.yml` desktop discovery with per-artifact fault isolation

The schema remains alpha. Compatibility and migration policy are V1-candidate decisions.

### Phase 4: GraphQL Adapter — Complete In Declared Scope

- query and mutation documents over HTTP with variables
- GraphQL error handling and response-data event emission
- local deterministic GraphQL fixture coverage

### Phase 5: WebSocket Adapter — Complete In Declared Scope

- `ws://` connection, JSON/text send, bounded receive, expectations, and event emission
- causal trace integration and local fixture coverage

### Phase 6: SNS/SQS Adapter — Complete In Declared Scope

- GoAWS-backed SNS publish and SQS send/receive/delete-on-success
- message-body and SNS envelope extraction through JSONPath
- causal trace integration and deterministic smoke coverage

### Phase 7: Safety And Run Planning — Complete In Declared Scope

- capability and risk reporting with deterministic effect order
- default denial of write-capable effects
- repeatable exact `--allow-capability` approvals and explicit `--allow-write`
- desktop confirmation passed to the engine as an exact per-run allowlist
- environment-aware plan, validate, and explicit dry-run preflight
- secret-reference interpolation and returned/persisted trace redaction

Secure OS-backed secret storage and protocol connectivity checks remain V1-candidate work.

### Phase 8: Desktop Workflow Bench — Complete As A Working Vertical Slice

- repository-confined workflow and environment discovery
- guarded YAML editing with validation, conflict detection, and safe replacement
- environment-aware planning and capability confirmation
- workflow execution plus searchable persisted trace history
- stale-request rejection and context locking across loading, validation, saving, and runs
- invalid-artifact isolation, retryable discovery errors, and hardened report ingestion

Native packaging and interactive packaged-app smoke testing remain release work.

## Active Milestone: V1 Candidate

The next work should be selected from these release and product gaps rather than adding more
bootstrap phases:

1. Freeze the V1 scope and compatibility promise for `devknife.workflow/v1alpha1`.
2. Build one production-shaped demo that communicates the event-native value without relying only
   on synthetic protocol fixtures.
3. Add native desktop packaging and an interactive Linux/Windows validation path.
4. Decide the V1 authentication and secret-storage boundary.
5. Decide which runtime controls enter V1: retries, cancellation, concurrency, and failure policy.
6. Document supported protocol boundaries, especially HTTP/TLS/auth and WebSocket variants.

OpenAPI import, Postman conversion, plugin APIs, hosted services, and broad protocol presets remain
post-V1 unless the scope is explicitly changed.

## V1 Candidate Exit Criteria

- [x] Event-native core proven through real local protocol I/O.
- [x] Narrow REST, GraphQL, SQS, and WebSocket support in the declared scope.
- [x] File-backed, versioned workflow/environment artifacts with strict validation.
- [x] Causal traces sufficient for local failure diagnosis.
- [x] Capability planning and exact approval enforcement.
- [x] Linux and Windows CI for the Rust workspace and desktop web build.
- [ ] Written V1 schema compatibility and migration policy.
- [ ] Production-shaped demonstration and acceptance script.
- [ ] Native desktop packages with interactive smoke validation.
- [ ] Explicit V1 decision for authentication and secure secret storage.

The unchecked criteria are the current release blockers.
