# ADR 0001: Event-Native Workflow Core

Status: Accepted

## Context

The project aims to model cross-protocol workflows with asynchronous behavior and causal debugging. A linear step list is often insufficient for branching, fan-out, retries, and async message flows.

## Decision

Use an event-native core model as the primary execution abstraction.

Canonical loop:

`event -> handler -> effect -> observation/result -> emitted events -> trace`

Linear workflows are represented as a constrained subset of this model.

## Consequences

Positive:

- supports async and branching naturally
- makes causality explicit
- aligns with trace-first debugging

Costs:

- more up-front modeling complexity than simple linear steps
- requires careful event schema and correlation discipline

## Alternatives Considered

- Pure linear chain model: simpler initially, but poor fit for async and fan-out.
- Script-first model (arbitrary code): flexible, but weak traceability and poor architectural guardrails.
- Hybrid with linear primary and events as optional addon: risks event model becoming second-class and inconsistent.
