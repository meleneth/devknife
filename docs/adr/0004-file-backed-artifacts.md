# ADR 0004: File-Backed Artifacts

Status: Accepted

## Context

Teams need reproducible, reviewable workflow definitions and run context. Hosted state as the source of truth adds friction for local development and versioned collaboration.

## Decision

Use file-backed artifacts as the source of truth for workflows, operations, environment definitions, and related project metadata.

Run traces are generated artifacts that can also be file-backed.

## Consequences

Positive:

- works naturally with git workflows
- enables code review and change history
- supports local-first use without hosted dependency

Costs:

- requires careful schema evolution strategy
- requires explicit handling of secrets outside shared files

## Alternatives Considered

- Hosted sync as source of truth: central management benefits, but higher coupling and weaker offline/local-first flow.
- Database-only local store: less transparent diffs and harder external tooling integration.
- Mixed source of truth (files plus hidden state): high confusion risk and drift.
