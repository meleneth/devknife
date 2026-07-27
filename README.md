# devknife (working title)

This repository defines a file-backed, cross-platform service workflow runner for developers. The core model is event-native execution: workflows start from seed events, handlers produce effects, effects produce observations, observations may emit more typed events, and every run yields a causal trace explaining what happened and why.

Status: Bootstrap implementation with narrow REST, GraphQL, SNS, SQS, and WebSocket adapters plus an initial Tauri/Vue desktop shell.

## What Exists In This Repository

- Product thesis and domain model docs.
- Explicit system invariants and non-goals.
- Architecture decision records (ADRs).
- Draft roadmap from bootstrap to v1.
- Devcontainer setup for Rust + Node toolchains.
- Rust workspace with a small event-native engine, CLI, YAML loader, typed trace, and narrow REST, GraphQL, SNS, SQS, and WebSocket effects.
- Tauri + Vue + shadcn-vue desktop scaffold that consumes the core through Tauri commands.

## What Does Not Exist Yet

- No plugin system.
- No hosted service.

## Open In Devcontainer

1. Install Docker Desktop (or another compatible container runtime).
2. Install VS Code with the Dev Containers extension.
3. Open this folder in VS Code.
4. Run: Dev Containers: Reopen in Container.
5. Verify toolchains:
   - `rustc --version`
   - `cargo --version`
   - `rustfmt --version`
   - `cargo clippy -V`
   - `node --version`
   - `npm --version`

See docs/008-devcontainer-and-tooling.md for details and troubleshooting.

## Read Next

1. docs/000-product-thesis.md
2. docs/002-invariants.md
3. docs/003-execution-model.md
4. docs/009-roadmap-bootstrap-to-v1.md
5. docs/010-open-questions.md

## Syntax Warning

Any syntax examples in this repository are illustrative draft shapes, not final language or schema commitments.

## Initial Implementation

The repository now contains a first Rust workspace with `devknife-core` and `devknife-cli`. The implemented engine is intentionally small: it runs in-memory `emit`, `record`, and event-payload `assert` effects, plus narrow real REST, GraphQL, SNS, SQS, and WebSocket effects that can call local HTTP JSON services, GoAWS, or a local WebSocket fixture, assert responses/messages, emit events from RFC 9535 JSONPath selectors such as `$.body.id` or `$.data.account.id`, and record the causal chain in the run trace.

Useful commands:

- `cargo test`
- `cargo run -p devknife-cli -- run examples/workflows/bootstrap.workflow.yaml`
- `cargo run -p devknife-cli -- run examples/workflows/bootstrap.workflow.yaml --json`
- `cargo run -p devknife-cli -- plan examples/workflows/bootstrap.workflow.yaml`
- `cargo run -p devknife-cli -- validate examples/workflows/bootstrap.workflow.yaml`
- `npm --prefix apps/desktop run build`
- `npm --prefix apps/desktop run dev:web`
- `npm --prefix apps/desktop run dev:tauri`
- `docker compose -f testbed/docker-compose.yml config`

`run` writes a stable JSON trace artifact to `runs/<run_id>.trace.json` by default. Use `--trace-dir <dir>` to choose another directory or `--no-trace-file` for stdout-only runs.
Write-capable effects are denied by default. Approve exact capabilities with repeatable
`--allow-capability <capability>` flags, or use `--allow-write` to approve every write capability
listed by the run plan.

The desktop app lives in `apps/desktop`. `dev:web` runs the Vue shell in a browser with fallback scaffold data; `dev:tauri` runs the desktop app and invokes Rust commands for workflow listing, planning, and execution. On Linux, Tauri requires WebKit/GTK system development packages; see `docs/008-devcontainer-and-tooling.md`.

REST smoke test:

- `docker compose -f testbed/docker-compose.yml up -d rest-service`
- `curl http://localhost:18101/health`
- `cargo run -p devknife-cli -- run examples/workflows/rest-smoke.workflow.yaml`
- `docker compose -f testbed/docker-compose.yml down`
- or `testbed/bin/rest-smoke`

GraphQL smoke test:

- `docker compose -f testbed/docker-compose.yml up -d graphql-service`
- `curl http://localhost:18102/health`
- `cargo run -p devknife-cli -- run examples/workflows/graphql-smoke.workflow.yaml --allow-write`
- `docker compose -f testbed/docker-compose.yml down`
- or `testbed/bin/graphql-smoke`

SNS/SQS smoke test:

- `docker compose -f testbed/docker-compose.yml up -d goaws`
- `cargo run -p devknife-cli -- run examples/workflows/sns-sqs-smoke.workflow.yaml --allow-write`
- `docker compose -f testbed/docker-compose.yml down`
- or `testbed/bin/sns-sqs-smoke`

WebSocket smoke test:

- `docker compose -f testbed/docker-compose.yml up --build -d websocket-service`
- `cargo run -p devknife-cli -- run examples/workflows/websocket-smoke.workflow.yaml --allow-write`
- `docker compose -f testbed/docker-compose.yml down`
- or `testbed/bin/websocket-smoke`

Cross-protocol smoke test:

- `testbed/bin/cross-protocol-smoke`

The local REST, GraphQL, GoAWS, and WebSocket fixtures are now used by the Rust engine.
