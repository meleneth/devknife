# 000 Product Thesis

Status: Draft

## Working Title

`devknife` is a temporary working title only.

## Problem Statement

Developers frequently need to exercise multi-step service behavior across API styles and transport styles. Existing tools are strong for one protocol at a time, but weak at causal, cross-protocol workflows that include asynchronous behavior.

The gap is not just making requests. The gap is proving cause and effect across services, messages, and live sessions.

## Product Thesis

Build a file-backed, cross-platform service workflow runner where REST calls, GraphQL operations, queue interactions, and live WebSocket sessions are driven by typed events and captured as a causal trace.

This product exists to answer:

- What happened?
- Why did it happen?
- Which previous event caused this effect?
- Which observation emitted the next event?

## Explicitly Not This

- Not a Postman clone.
- Not YAML shell scripts.
- Not Docker compose for requests.

## Durable Product Surface

The durable surface is:

- Workflow artifacts stored as files.
- A reusable engine that executes those artifacts.
- A causal trace artifact for each run.

The UI, when added later, is an interface over the engine and artifacts.

## Target Runtime Form

- Engine: Rust core.
- Initial interface: CLI first.
- Later interface: desktop UI (likely Tauri + Vue).

## Bootstrap Scope (Current Repo State)

This repository currently provides planning and project definition only:

- Thesis and domain docs.
- Invariants.
- ADRs.
- Roadmap.
- Devcontainer.

No engine implementation is included yet.

## Non-Goals For Bootstrap

- No desktop UI yet.
- No Tauri app yet.
- No workflow engine implementation yet.
- No plugin system yet.
- No scripting runtime yet.
- No hosted sync service.
- No Docker runtime requirement for users.
- No attempt to support every protocol.
- No first-class SOAP support.
- No database adapter yet.
- No Kafka/EventBridge/PubSub yet.
- No gRPC yet (conceptual chair remains open).
