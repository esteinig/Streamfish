use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    /// Represents a failure to obtain the port of a position likely due to that the position was not running at 
    /// initiation of the control server connection with the ManagerService
    #[error("Failed to obtain port of the requested position ({0})")]
    PortNotFound(String),
    // Represents a failure to obtain the read detection parameter from configuration in the AcquisitionService
    #[error("Failed to get the read detection parameters from configuration")]
    ReadDetectionParamsNotFound,
    // Represents failure to establish a channel with the control server due to invalid URI
    #[error("Failed to establish a channel due to invalid URI")]
    InvalidUri,
    // Represents failure to configure a secure channel to the control server with TLS
    #[error("Failed to configure a secure channel due to invalid configuration of TLS")]
    InvalidTls,
    // Represents failure to obtain a response from the control server endpoints
    #[error("Failed to obtain response from server (status: {0})")]
    ResponseError(String),
    // Represent a failure to initiate a connection with the control server
    #[error("Failed to initiate a connection to the control server")]
    ControlServerConnectionInitiation,
    // Represents failure to parse a response from the control server stream
    // this usually occurs because the connection has been terminated
    #[error("Control server connection has terminated")]
    ControlServerConnectionTermination,

    // Represent a failure to launch the Dori processing server as a task
    #[error("Failed to launch the processing server")]
    DoriServerLaunch,
    // Represent a failure to initiate a connection with the processing server
    #[error("Failed to initiate a connection to the processing server")]
    DoriServerConnectionInitiation,
    // Represents failure to parse a response from the processing server stream;
    // this usually occurs because the connection has been terminated
    #[error("Processing server connection has terminated")]
    DoriServerConnectionTermination,
    // Represents failure to send a decision action into the throttle queue
    #[error("Failed to send an decision action into the throttle queue")]
    DecisionQueueSend,
    // Represents failure to send a decision action into the logging queue
    #[error("Failed to send an decision action into the logging queue")]
    LoggingQueueSend,
    // Represents failure to send a decision action into the control server action queue
    #[error("Failed to send an decision action into the control server action queue")]
    ControlServerActionQueueSend,
    // Represents failure to send a decision action into the data processing stream queue
    #[error("Failed to send a data package into the processing server stream queue")]
    DoriStreamQueueSend,
    // Represents failure to send a termination signal into the shutdown queue
    #[error("Failed to send a termination signal into the shutdown queue")]
    ShutdownQueueSend,
    // Represents failure to create the logging file
    #[error("Failed to create the log file")]
    LogFileCreate,
    // Represents failure to write to log file
    #[error("Failed to create the log file")]
    LogFileWrite,
    // Represents failure to initiate the data acquisition stream with the control server
    #[error("Failed to initiate the stream with the control server")]
    ControlServerStreamInit,
    // Represents failure to send the initiation request to the control server
    #[error("Failed to send initiation request to the control server")]
    ControlServerStreamInitSend,
    // Represents failure to initiate the data processing stream with the processing server
    #[error("Failed to initiate the stream with the processing server")]
    DoriServerStreamInit,
    // Represents failure to send the initiation request to the processing server
    #[error("Failed to send initiation request to the processing server")]
    DoriServerStreamInitSend,
}