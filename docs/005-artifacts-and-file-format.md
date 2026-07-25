# 005 Artifacts And File Format

Status: Draft

## Principle

File-backed artifacts are the durable product surface. They should be versionable, diffable, reviewable, and executable.

Artifacts may expose familiar API-client affordances, but their shape should follow the native model: operation definitions, environments, event handlers, effects, observations, assertions, causal traces, capabilities, and file-backed organization. Postman collection compatibility may be served later by converters, not by making Postman shapes the artifact foundation.

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

## Implementation Update

Phase 1 introduced the Cargo workspace. `crates/devknife-core` owns the initial typed workflow model, in-memory event engine, trace model, run planning summary, and YAML loader. `crates/devknife-cli` owns the `devknife` binary for `run`, `plan`, and `validate`.

YAML is now the initial human-authored workflow format for bootstrap. The format is intentionally small and versioned as `devknife.workflow/v1alpha1`. The pipeline is YAML -> parsed config structs -> validation -> `Workflow` -> planning or engine execution.

Phase 2 adds the first real protocol artifact surface: REST effects in workflow YAML and named service bindings in environment YAML.

Current executable REST effect shape:

```yaml
version: devknife.workflow/v1alpha1
name: rest-smoke
handlers:
  - on: account.load.requested
    effects:
      - type: rest
        service: rest
        operation: get_account
        method: GET
        path: /accounts/{{ event.payload.account_id }}
        headers:
          x-correlation-id: "{{ event.payload.correlation_id }}"
        expect:
          status: 200
        emits:
          - event_type: account.loaded
            payload:
              account_id:
                from: $.body.id
```

Current environment binding shape:

```yaml
name: local
services:
  rest:
    base_url: http://localhost:18101
```

The interpolation and response path syntax are intentionally minimal. They are not a scripting language.
