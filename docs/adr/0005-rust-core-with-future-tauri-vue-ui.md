# ADR 0005: Rust Core With Future Tauri/Vue UI

Status: Accepted

## Context

The engine needs cross-platform performance, strong typing, and clear control over async behavior and protocol adapters. Desktop UI is expected later but should not duplicate core runtime logic.

## Decision

Use Rust for the core engine and CLI foundation. If a desktop app is pursued, prefer Tauri + Vue as an integration shell over the same engine.

No Tauri app is created in bootstrap.

## Consequences

Positive:

- strong type system for event/effect/observation invariants
- good cross-platform systems behavior
- desktop can reuse validated engine core

Costs:

- Rust learning curve
- UI integration complexity deferred to later phase

## Alternatives Considered

- Electron + Node core: faster UI momentum, weaker engine boundaries and potentially higher runtime overhead.
- CLI-only forever: reduces scope but may underserve trace visualization and editing ergonomics.
- Multi-language split core at bootstrap: too much complexity too early.
