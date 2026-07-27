# 004 Native Protocols

Status: Draft

This project treats protocol families as first-class domain concepts, not generic HTTP wrappers.

## REST

Implementation status: narrow real adapter.

Modeled with REST-native concepts:

- method
- URL/base URL/path
- path/query params
- headers
- body
- status
- response headers/body

Current implementation supports:

- named service binding to an HTTP base URL through environment YAML
- `GET`, `POST`, `PUT`, `PATCH`, and `DELETE`
- path, query params, headers, and optional JSON request body
- minimal string interpolation from `event.payload.*` and `env.*`
- typed response observation with status, headers, JSON body when possible, and text/empty fallback
- status equality assertion
- event emission from RFC 9535 JSONPath selectors such as `$.body.id`

Current limits:

- `http://` only; TLS support is future work
- no retries, timeout policies, auth helpers, cookies, multipart, or OpenAPI import
- no full expression language

OpenAPI import is a future capability.

## GraphQL

Implementation status: narrow real adapter.

Modeled with GraphQL-native concepts:

- operation name
- query vs mutation
- variables
- `data`
- `errors`
- `extensions`
- partial success behavior
- path-aware errors

GraphQL over HTTP 200 with `errors` must not be treated as generic success.

Current implementation supports:

- named service binding to an HTTP GraphQL URL through environment YAML
- query/mutation document submission over HTTP POST
- optional `operation_name`
- JSON variables with minimal interpolation from `event.payload.*` and `env.*`
- typed response observation with HTTP status, headers, `data`, `errors`, and `extensions`
- status equality assertion
- automatic failure when the GraphQL response contains `errors`, even with HTTP 200
- event emission from RFC 9535 JSONPath selectors such as `$.data.account.id`

Current limits:

- `http://` only; TLS support is future work
- no schema import, validation, persisted operations, fragments tooling, auth helpers, or retries
- no configurable partial-success policy yet

## SNS/SQS

Implementation status: narrow real adapter against GoAWS.

Modeled as asynchronous topic and queue semantics:

- publish
- send
- poll/receive
- match predicates
- visibility timeout awareness
- receipt handle awareness
- delete policy

V1 scope is intentionally narrower than full SQS semantics.

Current implementation supports:

- named service binding to a local GoAWS endpoint through environment YAML
- `sns_publish` to a topic ARN
- `sqs_send` to a queue URL
- `sqs_receive` from a queue URL with `max_messages`, `wait_time_seconds`, and `delete_on_success`

Workflow validation enforces SQS request limits (`max_messages` 1–10 and
`wait_time_seconds` 0–20) before execution. REST and GraphQL expected HTTP statuses must be in the
100–599 range.
Literal URLs and selected service bindings are also checked against the currently supported
schemes: `http://` for REST, GraphQL, SNS, and SQS; `ws://` for WebSocket.
- typed observations for publish/send/receive and received message metadata
- parsing JSON SQS bodies and GoAWS SNS notification envelopes
- convenience extraction from `$.message.body_message_json.*` when an SNS notification `Message` field contains JSON
- event emission from RFC 9535 JSONPath selectors over AWS operation/result documents

Current limits:

- GoAWS/local HTTP only; no AWS SigV4 or real AWS credentials yet
- no visibility timeout mutation, batch send/delete, FIFO-specific options, message attributes, or DLQ behavior
- receive polling is one query request, not a retry loop across multiple waits

## WebSockets

Implementation status: narrow real adapter against the local WebSocket fixture.

Modeled as live session semantics:

- named sessions
- send
- receive
- expect match with timeout
- emit event from observed message
- close
- concurrent observers while other effects execute

Current implementation supports:

- named service binding to a `ws://` URL through environment YAML
- direct `ws://` URL effects
- one connection per effect
- JSON or text sends
- one received message per effect
- `timeout_ms` for connect/write/read socket operations
- expectation checks from RFC 9535 JSONPath selectors over the received message
- event emission from RFC 9535 JSONPath selectors over the received message
- typed observations for send, receive, assertion, and connection failures

Current limits:

- `ws://` only; TLS support is future work
- no persistent session reuse yet, despite the `session` field being captured in trace output
- no concurrent observers, reconnect policies, ping/pong policy controls, or subprotocol negotiation
- no protocol presets yet for ActionCable, Socket.IO, GraphQL subscriptions, OBS WebSocket, or similar higher-level protocols

## Out of Scope For Bootstrap and Early V1

- SOAP first-class support
- gRPC adapter implementation
- Kafka/EventBridge/PubSub adapters
- database adapters

A conceptual chair remains available for future protocol adapters.
