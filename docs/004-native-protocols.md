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
- event emission from JSON response paths such as `body.id`

Current limits:

- `http://` only; TLS support is future work
- no retries, timeout policies, auth helpers, cookies, multipart, or OpenAPI import
- no full expression language

OpenAPI import is a future capability.

## GraphQL

Implementation status: future adapter, not executable yet.

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

## SQS

Implementation status: future adapter, not executable yet.

Modeled as asynchronous queue semantics:

- send
- poll/receive
- match predicates
- visibility timeout awareness
- receipt handle awareness
- delete policy

V1 scope is intentionally narrower than full SQS semantics.

## WebSockets

Implementation status: future adapter, not executable yet.

Modeled as live session semantics:

- named sessions
- connect
- send
- receive
- expect match with timeout
- emit event from observed message
- close
- concurrent observers while other effects execute

## Out of Scope For Bootstrap and Early V1

- SOAP first-class support
- gRPC adapter implementation
- Kafka/EventBridge/PubSub adapters
- database adapters

A conceptual chair remains available for future protocol adapters.
