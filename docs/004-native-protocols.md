# 004 Native Protocols

Status: Draft

This project treats protocol families as first-class domain concepts, not generic HTTP wrappers.

## REST

Modeled with REST-native concepts:

- method
- URL/base URL/path
- path/query params
- headers
- body
- status
- response headers/body

OpenAPI import is a future capability.

## GraphQL

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

Modeled as asynchronous queue semantics:

- send
- poll/receive
- match predicates
- visibility timeout awareness
- receipt handle awareness
- delete policy

V1 scope is intentionally narrower than full SQS semantics.

## WebSockets

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
