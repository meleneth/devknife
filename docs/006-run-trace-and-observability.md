# 006 Run Trace And Observability

Status: Draft

## Trace Is A Product Surface

Trace is not debug garnish. It is a core output artifact.

Every run should eventually explain:

- what happened
- why it happened
- what caused each effect
- what observations were produced
- which expectations passed or failed
- which values and references were used

## Trace Model (Conceptual)

Planned trace entry categories:

- run-start / run-end
- event-enqueued / event-dequeued
- handler-matched / handler-skipped
- effect-start / effect-end
- observation-recorded
- assertion-pass / assertion-fail
- event-emitted
- timeout
- cancellation

Each entry should include stable ids and causal links.

## Secret Hygiene In Trace

Trace may include secret reference names but never raw secret values.

## Human And Machine Consumption

The trace should support:

- human debugging in CLI/desktop views
- machine post-processing and filtering
- regression assertions in tests

## Future Trace Features

- compact vs verbose views
- redaction policy controls
- trace diff between runs
- timeline visualization in desktop UI

## Implementation Update

The Rust engine now records each executed effect as one trace entry containing:

- triggering event id
- handler index
- effect index
- typed effect
- typed observation

For REST effects, the observation includes the resolved method and URL, HTTP status, headers, parsed JSON body when available, assertion results, and any events emitted from response body fields. Emitted events carry `caused_by.event_id` and `caused_by.trace_entry_id`, preserving the path from seed event to REST call to response observation to follow-up event.

The CLI and desktop app write the full typed `RunReport` as stable JSON to
`runs/<run_id>.trace.json` by default. CLI users can use `--trace-dir <dir>` to change the
destination or `--no-trace-file` for stdout-only runs.

Failed assertions and failed protocol observations fail the run and leave the failed observation in the trace.
