# REST + GraphQL + SQS + WebSocket Scenario (Draft Syntax)

Status: Illustrative only. Syntax is not final.

## Intent

Show one workflow shape that spans all initial first-class protocols with causal event chaining.

```yaml
version: draft
workflow:
  id: multi-protocol-smoke
  seed_events:
    - type: run.begin
      payload:
        user_id: "u-42"

handlers:
  - on: { event_type: run.begin }
    effects:
      - effect_id: rest-create-order
        type: rest.request
        request:
          method: POST
          service: orders_api
          path: /orders
          body:
            user_id: "${event.payload.user_id}"
            run_id: "${context.run_id}"
        expect:
          status: 201
        emit_events:
          - event_type: order.created
            payload_from: "$.response.body"

  - on: { event_type: order.created }
    effects:
      - effect_id: graphql-load-order
        type: graphql.operation
        operation:
          operation_name: GetOrder
          query: |
            query GetOrder($id: ID!) {
              order(id: $id) { id state }
            }
          variables:
            id: "${event.payload.id}"
        expect:
          graphql:
            no_errors: true
        emit_events:
          - event_type: order.loaded
            payload_from: "$.graphql.data.order"

  - on: { event_type: order.loaded }
    effects:
      - effect_id: ws-connect
        type: websocket.connect
        session: order-updates
        endpoint: "${env.ws_base}/orders"
      - effect_id: ws-send-subscribe
        type: websocket.send
        session: order-updates
        message:
          action: subscribe
          order_id: "${event.payload.id}"
      - effect_id: ws-expect
        type: websocket.expect
        session: order-updates
        timeout_ms: 10000
        match:
          path: "$.run_id"
          equals: "${context.run_id}"
        emit_events:
          - event_type: order.ws_update
            payload_from: "$.message"

  - on: { event_type: order.ws_update }
    effects:
      - effect_id: sqs-send
        type: sqs.send
        queue: outbound-events
        body:
          run_id: "${context.run_id}"
          order_id: "${event.payload.order_id}"

      - effect_id: sqs-poll-match
        type: sqs.poll_match
        queue: inbound-events
        timeout_ms: 30000
        match:
          path: "$.run_id"
          equals: "${context.run_id}"
        on_match:
          delete_message: true
        emit_events:
          - event_type: sqs.matched
            payload_from: "$.message"
```

## Illustrative Causal Trace Snippet

```text
run.begin
  -> rest-create-order (POST /orders) status=201
    -> emitted order.created
      -> graphql-load-order (GetOrder) data.order.state="PENDING"
        -> emitted order.loaded
          -> ws-connect(session=order-updates)
          -> ws-send-subscribe
          -> ws-expect matched message run_id=run_123
            -> emitted order.ws_update
              -> sqs-send(queue=outbound-events)
              -> sqs-poll-match matched run_id=run_123 delete=true
                -> emitted sqs.matched
```
