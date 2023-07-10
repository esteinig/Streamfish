// Error for Dori server and client

use thiserror::Error;
use tonic::transport::Error as TransportError;

#[derive(Error, Debug)]
pub enum DoriError {
    // Represents failure to connect 
    #[error("failed to connect to Dori")]
    ConnectionFailure(#[from] TransportError),
}
