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
    #[error("failed to emit REST event {event_type}: response path '{path}' was not found")]
    RestEmissionPathMissing { event_type: String, path: String },
}
