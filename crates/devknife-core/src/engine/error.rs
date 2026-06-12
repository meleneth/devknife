use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EngineError {
    #[error("max event count exceeded: limit is {limit}")]
    MaxEventCountExceeded { limit: usize },
    #[error("max step count exceeded: limit is {limit}")]
    MaxStepCountExceeded { limit: usize },
    #[error("max event depth exceeded: limit is {limit}")]
    MaxDepthExceeded { limit: u32 },
}
