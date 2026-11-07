/// Bad edits and ambiguous paths.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    #[error("{0}")]
    Invalid(String),
    /// A path matching several nodes, with the full paths listed.
    #[error("{0}")]
    Ambiguous(String),
}

impl GraphError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        GraphError::Invalid(msg.into())
    }
}

pub type Result<T, E = GraphError> = std::result::Result<T, E>;
