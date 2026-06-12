require "json"
require "sinatra/base"

class RestService < Sinatra::Base
  set :show_exceptions, false

  ACCOUNTS = {
    "acct_001" => {
      id: "acct_001",
      name: "Demo Account",
      users: [
        { id: "user_001", email: "ava@example.test" },
        { id: "user_002", email: "sam@example.test" }
      ]
    }
  }.freeze

  before do
    content_type :json
  end

  get "/health" do
    JSON.generate(status: "ok", service: "rest-service")
  end

  get "/accounts/:id" do
    account = ACCOUNTS[params[:id]]
    halt 404, JSON.generate(error: "account not found") unless account

    JSON.generate(with_correlation(account.slice(:id, :name)))
  end

  post "/accounts" do
    request.body.rewind
    body = JSON.parse(request.body.read)
    id = "acct_created_001"

    JSON.generate(with_correlation(id: id, name: body.fetch("name")))
  rescue JSON::ParserError, KeyError => error
    status 400
    JSON.generate(error: error.message)
  end

  get "/accounts/:id/users" do
    account = ACCOUNTS[params[:id]]
    halt 404, JSON.generate(error: "account not found") unless account

    JSON.generate(with_correlation(account[:users]))
  end

  private

  def with_correlation(payload)
    correlation_id = request.env["HTTP_X_CORRELATION_ID"] || params["correlation_id"]
    return payload unless correlation_id

    payload.merge(correlation_id: correlation_id)
  end
end
