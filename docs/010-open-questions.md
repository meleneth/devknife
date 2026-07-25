# 010 Open Questions

Status: Active

## Format And Schema

- What should be the primary workflow authoring format: YAML, TOML, JSON, or mixed?
- How strict should payload schema enforcement be in v1?
- Should event types require registry declarations from day one?

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

- What is the first meaningful local demo scenario that proves event-native value?
- What is the practical local SQS test strategy (for example GoAWS or LocalStack later)?
- How do we keep Rust learning value high while using AI assistance responsibly?

## Product Boundary

- Where should CLI stop and desktop UI begin once both exist?
- Which capabilities are required by default vs opt-in per run?

## Postman-Class, Not Postman-Compatible

- Which familiar API client affordances should appear first: environments, variables, auth profiles, reusable operations, request history, response inspection, assertions/tests, or value extraction?
- What minimum native concepts are needed before Postman import/export can be useful as migration tooling?
- How should converters represent Postman collections and scripts without making them part of the core execution model?
- What compatibility boundaries should be documented so users understand this is better-than-Postman, not a drop-in Postman clone?

## Recently Narrowed

- YAML is the initial bootstrap workflow authoring format, with `devknife.workflow/v1alpha1` as the first explicit schema version. Strictness beyond current semantic validation remains open.
- GoAWS is the first local SQS fixture; whether LocalStack is needed later remains open.
- The first meaningful cross-protocol demo now runs against the local REST, GraphQL, WebSocket, and GoAWS fixtures. Next open question is what production-shaped demo should replace the synthetic fixture story.
