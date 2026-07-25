# devknife Local Testbed

This testbed provides deterministic local protocol fixtures for adapter work. The Rust engine now calls the REST and GraphQL services; WebSocket and GoAWS remain future adapter fixtures.

## Start

```sh
docker compose -f testbed/docker-compose.yml up --build
```

## Host Ports

- REST service: `http://localhost:18101`
- GraphQL service: `http://localhost:18102`
- WebSocket service: `ws://localhost:18103/ws`
- GoAWS SNS/SQS: `http://localhost:18104`

## GoAWS

Fake local credentials:

```sh
export AWS_ACCESS_KEY_ID=devknife
export AWS_SECRET_ACCESS_KEY=devknife
export AWS_REGION=us-east-1
export AWS_ENDPOINT_URL=http://localhost:18104
```

Deterministic local resources:

- Account ID: `100010001000`
- Region: `us-east-1`
- Workflow input queue URL: `http://localhost:18104/queue/devknife-workflow-input`
- Workflow results queue URL: `http://localhost:18104/queue/devknife-workflow-results`
- Event topic ARN: `arn:aws:sns:us-east-1:100010001000:devknife-events`

## Smoke Test

With the compose stack running:

```sh
testbed/bin/smoke
```

The smoke script requires `curl`. It uses Ruby from the host only for the WebSocket ping check.

REST-only engine smoke:

```sh
docker compose -f testbed/docker-compose.yml up -d rest-service
curl http://localhost:18101/health
cargo run -p devknife-cli -- run examples/workflows/rest-smoke.workflow.yaml
docker compose -f testbed/docker-compose.yml down
```

GraphQL-only engine smoke:

```sh
docker compose -f testbed/docker-compose.yml up -d graphql-service
curl http://localhost:18102/health
cargo run -p devknife-cli -- run examples/workflows/graphql-smoke.workflow.yaml
docker compose -f testbed/docker-compose.yml down
```

## Current Limits

- WebSocket and GoAWS fixtures are not wired into `devknife-core` yet.
- GoAWS configuration may need minor adjustment if the upstream image changes its config schema.
- WebSocket fixture is intentionally simple: JSON `ping` returns `pong`, JSON `subscribe` returns `subscription.confirmed`, and other JSON messages are echoed.
