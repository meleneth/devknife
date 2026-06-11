# 001 Domain Model

Status: Draft

## Core Concepts

- Project: top-level file-backed unit containing workflows, environment definitions, and event schema references.
- Workflow: event-native behavior graph, not primarily a linear script.
- Event: typed envelope carrying payload and causal metadata.
- Handler: rule that reacts to an event and chooses one or more effects.
- Effect: explicit external or internal action (REST call, GraphQL operation, SQS send/poll, WebSocket send/receive, emit event).
- Observation: result emitted from effect execution (response, message, timeout, assertion result).
- Trace Entry: immutable run record linking cause, effect, and observation.
- Capability: declared permission required for dangerous effects.
- Environment: named values and secret references used to bind workflows for execution.

## Event-Native Shape

Linear workflows are a subset. A linear chain can be represented as events where each step emits exactly one next-step event.

General case allows branching, fan-out, retry emission, timeout emission, and async correlations.

## Typed Envelopes

Event envelopes should be typed. Payloads may be JSON-like at first.

Good direction:

- `Event { event_type, payload, caused_by, emitted_by, run_id }`
- `Effect` and `Observation` represented by explicit discriminated variants.

Anti-direction:

- collapsing domain concepts into untyped generic value blobs.

## Causality

Every run artifact must be able to represent:

- triggering seed event(s)
- handler selection reason
- effect invocation parameters
- observation outcome
- emitted next events

This supports deterministic debugging and post-run auditing.

## Future Model Growth

Reserved conceptual chairs:

- gRPC adapter
- database adapters
- plugin adapters
- optional scripting escape hatch

These are not v1 commitments.
