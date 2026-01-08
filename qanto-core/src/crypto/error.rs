use thiserror::Error;

#[derive(Error, Debug)]
pub enum QanhashError {
    #[error("OpenCL error: {0}")]
    OpenClError(String),
    #[error("GPU error: {0}")]
    GpuError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("DAG generation error: {0}")]
    DagGenerationError(String),
    #[error("Other error: {0}")]
    Other(String),
}

impl From<String> for QanhashError {
    fn from(s: String) -> Self {
        QanhashError::Other(s)
    }
}

impl From<&str> for QanhashError {
    fn from(s: &str) -> Self {
        QanhashError::Other(s.to_string())
    }
}
