// Client implementations share similar setup and error codes from Minknow Instantiation 
// (except ManagerClient which needs to be initiated in the MinknowClient instantiation) 

use thiserror::Error;
use tonic::transport::Error as TransportError;
use tonic::codegen::http::uri::InvalidUri;

#[derive(Error, Debug)]
pub enum ClientError {
    /// Represents a failure to obtain the port of a position 
    /// likely due to that the position was not running at 
    /// initiation of the MinknowClient connection with the 
    /// ManagerService
    #[error("failed to obtain port of the requested position ({0})")]
    PortNotFound(String),
    // Represents failure to establish a channel due to invalid URI
    #[error("failed to configure a channel - invalid URI")]
    InvalidChannelUri(#[from] InvalidUri),
    // Represents failure to establish a channel due to invalid URI
    #[error("failed to configure a secure channel - invalid configuration of TLS")]
    InvalidTlsConfig(#[from] TransportError),
}


#[derive(Error, Debug)]
pub enum AnalysisConfigurationClientError {
    #[error("failed to get the read detection parameters from configuration")]
    ReadDetectionParamsNotFound,
}