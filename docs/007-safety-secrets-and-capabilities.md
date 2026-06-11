# 007 Safety, Secrets, And Capabilities

Status: Draft

## Safety Goals

- avoid accidental destructive actions
- make risky effects visible before execution
- keep secrets out of shared artifacts
- preserve reproducibility with explicit declarations

## Capabilities Model (Planned)

Dangerous effects require explicit capabilities.

Illustrative capability set:

- `rest.write`
- `graphql.mutate`
- `sqs.send`
- `sqs.delete`
- `websocket.send`
- `future.db.write`
- `future.shell.exec`

Runs should eventually display required capabilities before start.

## Risk Levels

Effects should declare risk level (for example: low, medium, high) with runner policy hooks.

## Secrets

Rules:

- secret values are local-only
- shared files contain references (for example `secret_ref: api_token`)
- trace never prints secret material

Cross-platform secret storage details are deferred and tracked in open questions.

## Dry-Run Intent

Where practical, effects should support dry-run behavior or preflight checks.

Dry-run is likely protocol-specific and must be explicit when unavailable.
