# ADR 0002: First-Class REST, GraphQL, SQS, WebSocket

Status: Accepted

## Context

The product thesis requires native handling across synchronous request/response and asynchronous session/message flows. A generic HTTP abstraction would flatten important protocol semantics.

## Decision

Model REST, GraphQL, SQS, and WebSockets as first-class effect/observation families.

No protocol is represented as a generic stringly transport placeholder.

## Consequences

Positive:

- preserves protocol-specific correctness
- enables clearer assertions and trace semantics
- avoids semantic loss for GraphQL errors, queue lifecycle, and WebSocket sessions

Costs:

- larger initial adapter surface
- more explicit schema and validation paths

## Alternatives Considered

- Generic HTTP model for all: fastest start, highest semantic loss.
- REST-first only, add others later without native model: leads to migration debt and broken abstractions.
- Plugin-only protocols from day one: too much complexity before core shape is proven.
