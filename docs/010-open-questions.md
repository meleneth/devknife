# 010 Open Questions

Status: Active

## Format And Schema

- How strict should payload schema enforcement be in v1?
- Should event types require registry declarations from day one?
- What compatibility and migration promise should `devknife.workflow/v1alpha1` make?

## Protocol Integration Order

- Should OpenAPI import arrive before or after hand-authored REST operations?
- How should GraphQL schema import and validation be staged?

## WebSocket Abstractions

- How should protocol presets be represented (plain JSON, ActionCable, Socket.IO, GraphQL subscriptions, OBS WebSocket)?
- Should presets be built-in, optional adapters, or external plugins?

## Secrets And Local Storage

- What cross-platform local secret store strategy should be used on Linux/Windows?
- How should secret rotation and environment overrides be represented?

## Runtime And Extensibility

- How much concurrency belongs in v1?
- What plugin/adapter model should exist later, and when should it be introduced?
- How can we preserve declarative-first design without turning files into YAML shell scripts?

## Developer Experience

- How polished should the customer onboarding demo be before V1 release notes and screenshots?
- How do we keep Rust learning value high while using AI assistance responsibly?

## Product Boundary

- Where should CLI stop and desktop UI begin once both exist?

## Postman-Class, Not Postman-Compatible

- Which familiar API client affordances should appear first: environments, variables, auth profiles, reusable operations, request history, response inspection, assertions/tests, or value extraction?
- What minimum native concepts are needed before Postman import/export can be useful as migration tooling?
- How should converters represent Postman collections and scripts without making them part of the core execution model?
- What compatibility boundaries should be documented so users understand this is better-than-Postman, not a drop-in Postman clone?

## Recently Narrowed

- YAML is the initial bootstrap workflow authoring format, with `devknife.workflow/v1alpha1` as the first explicit schema version. Strictness beyond current semantic validation remains open.
- GoAWS is the first local SQS fixture; whether LocalStack is needed later remains open.
- The first meaningful cross-protocol demo runs against local REST, GraphQL, WebSocket, and GoAWS
  fixtures. A chained REST fixture also passes a created account ID into a subsequent user-creation
  API. The V1 acceptance story is now the customer onboarding demo in `examples/workflows/customer-onboarding-demo.workflow.yaml`.
- Write-capable effects are opt-in per run. Exact capability approvals are the default policy;
  approve-all remains an explicit convenience.
