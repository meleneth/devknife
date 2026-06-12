require "graphql"
require "json"
require "sinatra/base"

DATA = {
  "acct_001" => {
    id: "acct_001",
    name: "Demo Account",
    users: [
      { id: "user_001", email: "ava@example.test" },
      { id: "user_002", email: "sam@example.test" }
    ]
  }
}

class UserType < GraphQL::Schema::Object
  field :id, ID, null: false
  field :email, String, null: false
end

class AccountType < GraphQL::Schema::Object
  field :id, ID, null: false
  field :name, String, null: false
  field :users, [UserType], null: false
end

class QueryType < GraphQL::Schema::Object
  field :account, AccountType, null: true do
    argument :id, ID, required: true
  end
  field :accounts, [AccountType], null: false

  def account(id:)
    DATA[id]
  end

  def accounts
    DATA.values
  end
end

class MutationType < GraphQL::Schema::Object
  field :create_account, AccountType, null: false do
    argument :name, String, required: true
    argument :correlation_id, String, required: false
  end

  def create_account(name:, correlation_id: nil)
    {
      id: "acct_created_001",
      name: name,
      users: [],
      correlation_id: correlation_id
    }
  end
end

class TestbedSchema < GraphQL::Schema
  query QueryType
  mutation MutationType
end

class GraphqlService < Sinatra::Base
  before do
    content_type :json
  end

  get "/health" do
    JSON.generate(status: "ok", service: "graphql-service")
  end

  post "/graphql" do
    request.body.rewind
    body = JSON.parse(request.body.read)
    result = TestbedSchema.execute(
      body.fetch("query"),
      variables: body["variables"] || {},
      operation_name: body["operationName"]
    )
    JSON.generate(result.to_h)
  rescue JSON::ParserError, KeyError => error
    status 400
    JSON.generate(errors: [{ message: error.message }])
  end
end
