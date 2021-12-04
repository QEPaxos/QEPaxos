use std::error::Error;
use std::io::Error as IoError;
use thiserror::Error as ThisError;
use tokio::sync::mpsc::error::SendError;
use tonic::Status;

#[derive(Debug, ThisError)]
pub enum SEPaxosError {
    #[error("Io Error {0}")]
    Io(#[from] IoError),
    #[error("grpc Error {0}")]
    Tonic(#[from] Status),
    #[error("client channel send error {0}")]
    ClientChannelError(#[from] SendError<bool>),
    #[error(" {0} error")]
    Common(String),
    #[error("{0}")]
    Other(#[from] Box<dyn Error + Send + Sync>),
}

pub type Result<T> = ::std::result::Result<T, SEPaxosError>;

impl From<String> for SEPaxosError {
    fn from(err: String) -> Self {
        SEPaxosError::Common(err)
    }
}
