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

- Not a drop-in Postman clone whose success is measured by Postman collection compatibility.
- Not YAML shell scripts.
- Not Docker compose for requests.

## Postman-Class, Not Postman-Compatible

This product should be Postman-class, but not Postman-compatible by default. It should feel understandable to developers who have used Postman, Insomnia, Bruno, curl, or internal API consoles without adopting any one tool's native formats, scripting model, or sync assumptions as the core architecture.

Familiar affordances are not architectural surrender. Useful API-client concepts should not be rejected merely because Postman also has them, and recognizable Postman concepts should not be inherited merely because users know them. Every familiar concept must be re-expressed through this project's native model: operation definitions, environments, event handlers, effects, observations, assertions, causal traces, capabilities, and file-backed artifacts.

Familiar API client affordances are intentionally in scope:

- Projects or workspaces.
- Collections or collection-like grouping.
- Environments and variables.
- Local secrets.
- Auth profiles.
- Reusable requests and operations.
- Request history.
- Response inspection.
- Assertions and tests.
- Extracting response values into later workflow steps.
- Sharing workflow artifacts through files and version control.
- Import/export eventually.

Postman-specific assumptions are intentionally out of scope for the native model:

- Postman's collection format is not the native workflow model.
- Postman compatibility is not a bootstrap requirement.
- Postman's scripting model is not the execution model.
- Hosted sync is not the source of truth.
- Drop-in replacement compatibility is not the success metric.

The distinction is architectural. Native artifacts should be event-oriented, file-backed, typed where it matters, and traceable. Shaping the core model around Postman collection compatibility would pull the product toward request lists, format constraints, and scripting assumptions instead of causal workflows across REST, GraphQL, queues, and live sessions.

Future Postman import/export converters may be useful migration tooling. They should live at the boundary: translating into and out of the native domain model without defining that model or contaminating the engine's internal concepts.

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

This repository currently provides:

- Thesis and domain docs.
- Invariants.
- ADRs.
- Roadmap.
- Devcontainer.
- Rust core and CLI.
- YAML workflow and environment loading.
- Trace artifacts.
- Advisory run planning.
- Narrow REST, GraphQL, SNS/SQS, and WebSocket adapters.
- Local deterministic protocol fixtures.

## Non-Goals For Bootstrap

- No desktop UI yet.
- No Tauri app yet.
- No plugin system yet.
- No scripting runtime yet.
- No hosted sync service.
- No Docker runtime requirement for users.
- No Postman collection compatibility requirement.
- No Postman scripting compatibility layer.
- No attempt to support every protocol.
- No first-class SOAP support.
- No database adapter yet.
- No Kafka/EventBridge/PubSub yet.
- No gRPC yet (conceptual chair remains open).
