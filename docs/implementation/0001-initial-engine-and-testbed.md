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
  - `devknife plan <workflow>`
  - `devknife validate <workflow>`
- Executable bootstrap workflow at `examples/workflows/bootstrap.workflow.yaml`.
- Executable cross-protocol workflow at `examples/workflows/cross-protocol-smoke.workflow.yaml`.
- Docker Compose testbed for protocol adapter work.

## What Remains Illustrative Only

- Secret handling
- Capability enforcement

## Run The CLI

```sh
cargo run -p devknife-cli -- run examples/workflows/bootstrap.workflow.yaml
cargo run -p devknife-cli -- run examples/workflows/bootstrap.workflow.yaml --json
cargo run -p devknife-cli -- plan examples/workflows/bootstrap.workflow.yaml
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

The testbed is consumed by the Rust engine for focused REST, GraphQL, SNS/SQS,
WebSocket, and cross-protocol smoke workflows.

## Known Limitations

- YAML schema is intentionally small and currently versioned as `devknife.workflow/v1alpha1`.
- Assertion paths support simple dot-separated payload keys only.
- Run IDs are UUIDs; event and trace ordering is deterministic within a run.
- The core is synchronous.
- Protocol-specific effects are currently limited to narrow REST, GraphQL, SNS, SQS, and WebSocket adapters.
