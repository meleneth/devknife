# 002 System Invariants

Status: Draft

These invariants are architectural constraints, not feature wishes. The roadmap may change. These constraints should survive roadmap changes.

## 1) Event-native core

Execution loop:

`event -> handler -> effect -> observation/result -> emitted events -> trace`

A workflow is not primarily a linear script.

## 2) Protocols stay first-class

- REST is first-class.
- GraphQL is first-class, not a generic HTTP blob.
- SQS is first-class async transport, not fake request/response.
- WebSockets are first-class sessions, not delayed HTTP.

## 3) Engine owns execution truth

The engine is source of truth. CLI and desktop UI are clients of the same core.

## 4) File-backed artifacts are durable

Workflows, operations, environments, and related definitions are file-backed, diffable, reviewable, and shareable through version control.

## 5) Secrets are not shared artifacts

Shared files may reference secret names. Secret material lives outside versioned workflow artifacts.

## 6) Effects are explicit and traceable

Each effect must declare:

- type
- inputs
- outputs/result shape
- auth/secrets needed
- risk level
- dry-run behavior where possible
- trace representation

No hidden side effects.

## 7) Dangerous effects require capabilities

Mutating effects require declared capabilities and should be visible in run planning.

## 8) Causal trace is first-class

A run must answer what happened, why, and how values/events/expectations/secrets references participated.

## 9) Declarative before scripting

Common behavior should be declarative and traceable. Scripting, if ever added, is an escape hatch.

## 10) Typed envelopes, flexible payloads

Payload flexibility is acceptable. Domain collapse into untyped blobs is not.

## 11) Async correlation is mandatory

Plan for `run_id`, correlation fields, match predicates, timeout policy, and provenance.

## 12) WebSocket model includes sessions and expectations

Must represent connect/send/receive/expect-with-timeout/emit-event/close and active observers.

## 13) SQS model includes lifecycle semantics

V1 narrow scope:

- send
- poll until match
- optional delete on success
- emit event from matched message

Future scope includes DLQ/FIFO/details.

## 14) GraphQL semantics stay GraphQL-native

Must model operation shape, variables, `data`, `errors`, `extensions`, partial success, path-aware failures.

## 15) REST semantics stay REST-native

Must model method, URL decomposition, params, headers, body, status, response headers/body.

## 16) Execution policy must be explicit

No hidden defaults for concurrency, retries, deduplication, max events, cancellation, timeout, failure policy, and cleanup.

## 17) Cross-platform means Linux and Windows

Docker is allowed for development but not required for end-user runtime.

## 18) AI-assisted development guardrails

- small milestones
- compile early and often
- tests before expansion
- reject blob shortcuts
- avoid overuse of `Arc<Mutex<_>>`
- avoid clone storms as architecture
- avoid giant async functions
- avoid `Box<dyn Any>` style escape hatches for core model
- keep protocol edges separate from workflow core
- convert invariants into executable tests as implementation begins
