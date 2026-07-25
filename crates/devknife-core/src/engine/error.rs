use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("max event count exceeded: limit is {limit}")]
    MaxEventCountExceeded { limit: usize },
    #[error("max step count exceeded: limit is {limit}")]
    MaxStepCountExceeded { limit: usize },
    #[error("max event depth exceeded: limit is {limit}")]
    MaxDepthExceeded { limit: u32 },
    #[error("missing REST service binding for '{service}'")]
    MissingRestServiceBinding { service: String },
    #[error("REST effect requires service or base_url")]
    MissingRestBaseUrl,
    #[error(
        "unsupported REST base URL '{base_url}': only http:// URLs are supported in this phase"
    )]
    UnsupportedRestBaseUrl { base_url: String },
    #[error("failed to build REST request for {operation}: {message}")]
    RestRequestBuild { operation: String, message: String },
    #[error("failed to execute REST operation {operation}: {message}")]
    RestRequestFailed { operation: String, message: String },
    #[error("failed to emit REST event {event_type}: JSONPath '{path}' did not match")]
    RestEmissionPathMissing { event_type: String, path: String },
    #[error("failed to emit REST event {event_type}: JSONPath '{path}' is invalid: {message}")]
    RestEmissionPathInvalid {
        event_type: String,
        path: String,
        message: String,
    },
    #[error("missing GraphQL service binding for '{service}'")]
    MissingGraphqlServiceBinding { service: String },
    #[error("GraphQL effect requires service or base_url")]
    MissingGraphqlBaseUrl,
    #[error(
        "unsupported GraphQL base URL '{base_url}': only http:// URLs are supported in this phase"
    )]
    UnsupportedGraphqlBaseUrl { base_url: String },
    #[error("failed to build GraphQL request for {operation}: {message}")]
    GraphqlRequestBuild { operation: String, message: String },
    #[error("failed to execute GraphQL operation {operation}: {message}")]
    GraphqlRequestFailed { operation: String, message: String },
    #[error("failed to emit GraphQL event {event_type}: JSONPath '{path}' did not match")]
    GraphqlEmissionPathMissing { event_type: String, path: String },
    #[error("failed to emit GraphQL event {event_type}: JSONPath '{path}' is invalid: {message}")]
    GraphqlEmissionPathInvalid {
        event_type: String,
        path: String,
        message: String,
    },
}
