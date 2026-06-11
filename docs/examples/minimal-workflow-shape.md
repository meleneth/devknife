# Minimal Workflow Shape (Draft Syntax)

Status: Illustrative only. Syntax is not final.

## Intent

Show the smallest event-native shape:

- one seed event
- one handler
- one internal emit effect
- traceable causality

```yaml
version: draft
workflow:
  id: hello-minimal
  seed_events:
    - type: app.start
      payload:
        message: "hello"

handlers:
  - on:
      event_type: app.start
    effects:
      - effect_id: emit-1
        type: emit_event
        emit:
          event_type: app.started
          payload:
            echoed: "${event.payload.message}"

  - on:
      event_type: app.started
    effects:
      - effect_id: assert-1
        type: assert
        expect:
          path: "$.event.payload.echoed"
          equals: "hello"
```

## Illustrative Trace Snippet

```json
{
  "run_id": "run_123",
  "entries": [
    {
      "kind": "event_dequeued",
      "event_type": "app.start"
    },
    {
      "kind": "effect_end",
      "effect_id": "emit-1",
      "emitted_events": ["app.started"]
    }
  ]
}
```
