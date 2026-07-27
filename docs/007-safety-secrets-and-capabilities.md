# 007 Safety, Secrets, And Capabilities

Status: Draft

## Safety Goals

- avoid accidental destructive actions
- make risky effects visible before execution
- keep secrets out of shared artifacts
- preserve reproducibility with explicit declarations

## Capabilities Model

Dangerous effects require explicit capabilities.

Current run planning reports required capabilities before execution:

- `workflow.emit`
- `workflow.record`
- `workflow.assert`
- `network.http.read`
- `network.http.write`
- `network.graphql`
- `aws.sns.publish`
- `aws.sqs.send`
- `aws.sqs.receive`
- `aws.sqs.delete`
- `network.websocket`
- `future.db.write`
- `future.shell.exec`

`devknife plan <workflow>` displays required capabilities and effect order, and
`devknife run --show-plan <workflow>` can print the same summary before execution. CLI runs deny
write-capable effects unless each one is approved with a repeatable
`--allow-capability <capability>` option. `--allow-write` remains available as an explicit
approve-all convenience. The desktop requires explicit confirmation for write-capable effects.

## Risk Levels

Effects should declare risk level (for example: low, medium, high) with runner policy hooks.

## Secrets

Rules:

- secret values are local-only
- shared files contain references (for example `secret_ref: api_token`)
- trace never prints secret material

Environment `secret_refs` can be referenced in workflow templates as `{{ secret.name }}`. Secret
values are redacted from returned and persisted run reports. Environment files remain local
configuration and must not contain production secrets in version control.

Cross-platform secret-store integration details are deferred and tracked in open questions.

## Dry-Run Intent

Where practical, effects should support dry-run behavior or preflight checks.

Dry-run is likely protocol-specific and must be explicit when unavailable.
