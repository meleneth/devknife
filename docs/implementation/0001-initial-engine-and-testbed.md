# 0001 Initial Engine And Testbed

Status: Implemented

## What Was Added

- Rust workspace with `devknife-core` and `devknife-cli`.
- Typed event, effect, observation, workflow, and trace structs.
- Deterministic serial in-memory engine loop.
- Bootstrap effects: `emit`, `record`, and `assert`.
- Guardrails for max events, max steps, and max depth.
- YAML workflow loading through parsed config structs and semantic validation.
- CLI commands:
  - `devknife run <workflow>`
  - `devknife run <workflow> --json`
  - `devknife validate <workflow>`
- Executable bootstrap workflow at `examples/workflows/bootstrap.workflow.yaml`.
- Draft future workflow showing REST, GraphQL, SQS, and WebSocket intent.
- Docker Compose testbed for future protocol adapter work.

## What Remains Illustrative Only

- `examples/workflows/rest-graphql-sqs-websocket.future.workflow.yaml`
- Environment binding in `examples/environments/local.yaml`
- SQS and WebSocket effects in the engine
- Secret handling and capabilities
- Generated run trace files on disk

## Run The CLI

```sh
cargo run -p devknife-cli -- run examples/workflows/bootstrap.workflow.yaml
cargo run -p devknife-cli -- run examples/workflows/bootstrap.workflow.yaml --json
cargo run -p devknife-cli -- validate examples/workflows/bootstrap.workflow.yaml
```

## Run Tests And Checks

```sh
cargo fmt
cargo test
cargo clippy --all-targets --all-features
```

## Start Or Inspect The Testbed

```sh
docker compose -f testbed/docker-compose.yml config
docker compose -f testbed/docker-compose.yml up --build
```

The testbed is intentionally not consumed by the Rust engine yet. It exists so
future REST, GraphQL, SQS, and WebSocket adapters have stable local targets.

## Known Limitations

- YAML schema is intentionally small and not versioned yet.
- Assertion paths support simple dot-separated payload keys only.
- Run IDs are UUIDs; event and trace ordering is deterministic within a run.
- The core is synchronous because current effects are in-memory only.
- Protocol-specific effects are currently limited to narrow REST and GraphQL adapters.
