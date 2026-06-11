# 005 Artifacts And File Format

Status: Draft

## Principle

File-backed artifacts are the durable product surface. They should be versionable, diffable, reviewable, and executable.

## Artifact Families (Planned)

- project definition
- workflow definitions
- operation definitions (REST/GraphQL/SQS/WebSocket)
- environment definitions
- event schema references
- run traces (generated artifacts)

## File Format Decision (Deferred)

Exact syntax is intentionally deferred to avoid premature lock-in.

Candidate directions:

- YAML
- TOML
- JSON
- mixed model (human-authored YAML/TOML + generated JSON traces)

See open questions for finalization criteria.

## Design Constraints For Any Chosen Format

- strongly named sections, not free-form script blocks
- explicit effect and expectation blocks
- explicit event emission mapping
- secret references by name only
- stable identifiers for trace linking
- machine-validated shape with useful diagnostics

## Minimal Validation Expectations

Even in early parser stages, validation should check:

- required identifiers
- event type references
- effect type support
- obvious shape errors
- missing environment bindings

## Why No Cargo Workspace Yet

No Cargo workspace is created in bootstrap because the implementation plan is still being established through invariants and ADRs. Creating crates now would imply architecture certainty that does not yet exist. The roadmap defines when to introduce workspace layout (Phase 1).
