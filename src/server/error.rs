// Error for Dori server and client

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    // Represents failure to establish a channel with the control server due to invalid URI
    #[error("Failed to establish a channel with the processing server due to invalid URI")]
    InvalidUri,
    // Represents failure to configure a valid address for connection with the processing server
    #[error("Failed to parse a valid address from the provided host and port of the processing server")]
    InvalidSocketAddress,
    // Represent a failure to serve the processing server with a TCP connection
    #[error("Failed to serve the processing server on TCP")]
    ServeTcp,
    // Represent a failure to serve the processing server with a UDS connection
    #[error("Failed to serve the processing server on UDS")]
    ServeUds,
    // Represent a failure to binda listener to the domain socket
    #[error("Failed to bind a listener to a domain socket")]
    UnixDomainSocketListener,
    // Represent a failure to remove a domain socket path
    #[error("Failed to remove a domain socket path")]
    UnixDomainSocketRemove,
    // Represent a failure to create the parent directory of a domain socket path
    #[error("Failed to create parent directory of a domain socket path")]
    UnixDomainSocketParentDirectoryCreate,
    // Represent a failure to obtain the parent directory path of a domain socket
    #[error("Failed to obtain parent directory of a domain socket path")]
    UnixDomainSocketParentDirectoryPath,
}
