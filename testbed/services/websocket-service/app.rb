require "faye/websocket"
require "json"
require "rack"

class WebsocketService
  KEEPALIVE_SECONDS = 15

  def call(env)
    request = Rack::Request.new(env)

    return health if request.get? && request.path == "/health"
    return websocket(env) if request.path == "/ws" && Faye::WebSocket.websocket?(env)

    [404, { "content-type" => "application/json" }, [JSON.generate(error: "not found")]]
  end

  private

  def health
    [200, { "content-type" => "application/json" }, [JSON.generate(status: "ok", service: "websocket-service")]]
  end

  def websocket(env)
    ws = Faye::WebSocket.new(env, nil, ping: KEEPALIVE_SECONDS)

    ws.on :message do |event|
      ws.send(JSON.generate(response_for(event.data)))
    end

    ws.rack_response
  end

  def response_for(data)
    payload = JSON.parse(data)
    correlation_id = payload["correlation_id"]

    case payload["type"]
    when "ping"
      { type: "pong", correlation_id: correlation_id }
    when "subscribe"
      {
        type: "subscription.confirmed",
        channel: payload["channel"],
        correlation_id: correlation_id
      }
    else
      {
        type: "echo",
        payload: payload,
        correlation_id: correlation_id
      }
    end
  rescue JSON::ParserError => error
    { type: "error", message: error.message }
  end
end
