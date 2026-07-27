# 003 Execution Model

Status: Draft

## Canonical Loop

The runtime loop is event-oriented:

1. seed event(s) enter queue
2. handler(s) match event type and predicates
3. selected handler emits one or more effects
4. effects execute and yield observations/results
5. observations may trigger assertions and event emission
6. emitted events continue execution
7. all steps append to causal trace

## Event Processing Concepts

- Event Envelope: typed metadata + flexible payload.
- Handler Match: deterministic criteria (event type, optional predicates).
- Effect Dispatch: protocol-native execution path.
- Observation Mapping: protocol response/message to typed observation.
- Event Emission: explicit mapping from observation to next event(s).

## Execution Policies

The current runner is deterministic and serial. It implements a maximum event count, bounded
protocol waits/timeouts, and exact capability allowlists. Write-capable effects are denied unless
approved for the run.

The following policy controls remain open for V1 scope:

- serial-only vs concurrent handler execution
- retry policy
- deduplication strategy
- cancellation behavior
- fail-fast vs continue-with-errors

## Assertions and Expectations

Effects and observations can include expectations:

- value assertions
- status assertions
- match predicates
- timeout expectations

Failures become first-class trace records, not silent logs.

## Cleanup / Finalization

Finalizers are a future concern for resources such as sessions and message handles. Bootstrap docs establish the conceptual requirement only.
