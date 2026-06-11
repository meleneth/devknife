# ADR 0003: CLI Core Before Desktop UI

Status: Accepted

## Context

The engine and artifact model must be the durable core. UI should not become execution truth. Early implementation needs fast feedback loops and low complexity.

## Decision

Build and validate the core through CLI-first execution before desktop UI work.

Desktop UI (likely Tauri + Vue) is deferred until engine capabilities and trace model are stable enough to consume.

## Consequences

Positive:

- keeps focus on engine correctness
- faster testing and CI path
- avoids UI-driven architecture distortion

Costs:

- delayed visual workflow experience
- less immediate discoverability for non-CLI users

## Alternatives Considered

- Desktop-first: better visual momentum, higher risk of UI becoming accidental source of truth.
- CLI-only permanently: simpler, but leaves trace exploration and project editing UX opportunities underdeveloped.
- Build both simultaneously: likely slower and more fragmented during bootstrap.
