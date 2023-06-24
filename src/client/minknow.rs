


use thiserror::Error;
use std::collections::HashMap;
use tonic::transport::{ClientTlsConfig, Certificate, Channel};


use crate::client::services::data::DataClient;
use crate::client::services::manager::ManagerClient;
use crate::{config::MinKnowConfig, services::minknow_api::manager::FlowCellPosition};



#[derive(Error, Debug)]
pub enum ActivePositionError {
    /// Represents a failure to obtain the port of a position - this
    /// usually occurs when the position is not in a running state
    #[error("failed to obtain port of the requested position ({0})")]
    PortNotFound(String),
    /// Represents a failure to obtain a position - this
    /// likely due to that the position was not running at 
    /// initiation of the MinKnowClient connection with the 
    /// ManagerService
    #[error("failed to obtain the requested position ({0})")]
    PositionNotFound(String),
}

pub struct ActiveChannels {
    pub manager: Channel
}

pub struct ActiveClients {
    pub manager: ManagerClient
}

pub struct ActivePositions {
    pub positions: HashMap<String, FlowCellPosition>,
}
impl ActivePositions {
    pub fn get_secure_port(&self, name: &str) -> Result<u32, ActivePositionError> {
        match self.positions.get(name) {
            Some(position) => match &position.rpc_ports {
                Some(rpc_ports) => Ok(rpc_ports.secure),
                None => Err(ActivePositionError::PortNotFound(name.to_string()))
            },
            None => Err(ActivePositionError::PositionNotFound(name.to_string()))
        }
    }
    pub fn get_position(&self, name: &str) -> Result<FlowCellPosition, ActivePositionError> {
        match self.positions.get(name) {
            Some(position) => Ok(position.clone()),
            None => Err(ActivePositionError::PositionNotFound(name.to_string()))
        }
    }
}

// Main client for the MinKnow API
pub struct MinKnowClient{
    pub tls: ClientTlsConfig,
    pub config: MinKnowConfig,
    pub clients: ActiveClients,
    pub channels: ActiveChannels,
    pub positions: ActivePositions
}

impl MinKnowClient {

    pub async fn connect(config: &MinKnowConfig) -> Result<Self, Box<dyn std::error::Error>> {

        // When connecting to MinKnow we require a secure channel (TLS). However, we were getting 
        // an error through the underlying TLS certificate library, solution is documented here.
        //
        // Error: InvalidCertificate(Other(UnsupportedCertVersion)) 
        // 
        // The error code from the `webpki` library states:
        //
        //      The certificate is not a v3 X.509 certificate.
        //
        // Looks like MinKnow is using outdated certificate versions. This issue is relevant and has
        // a solution:
        //
        //      https://github.com/rustls/rustls/issues/127
        //
        // However, we need to modify the ClientTlsConfig in `tonic` to disable certificate verifiction.
        //
        // The insane thing is that this actually worked thanks to the commenter in the issue... holy fuck,
        // this is such a hack. It now requires a modified version of `tonic` which is available as a fork
        // on my GitHub (https://github.com/esteinig/tonic @ v0.9.2-r1)

        let cert = std::fs::read_to_string(&config.tls_cert_path).expect(
            &format!("Failed to read certificate from path: {}", &config.tls_cert_path.display()) 
        );
        let tls = ClientTlsConfig::new()
            .domain_name("localhost")
            .ca_certificate(Certificate::from_pem(cert));


        // ========================
        // ManagerClient Initiation 
        // ========================

        // Establish a secure channel to the MinKnow manager service, that will be 
        // available throughout to request data from the manager service
        let manager_channel = Channel::from_shared(
            format!("https://{}:{}", config.host, config.port)
        )?.tls_config(tls.clone())?.connect().await?;

        // Use a simple interceptor for authentication - this might have to be generalised
        // using `tower` middleware - but it's too complex for my simple brain right now.

        // We clone the established channel (cheap, see below) and instantiate a new manger 
        // service client, that allows us to send the implemented requests. This is a general
        // patterns to create the client wrappers.
        
        let mut manager_client = ManagerClient::new(
            manager_channel.clone(), config.token.clone()
        );

        // Get the version information to test the connection and print the version of MinKnow
        let version_response = manager_client.get_version_info().await?;
        log::info!("MinKnow version: v{}", version_response);

        // Get the flowcell positions and print their total count, if present, print their summary
        let position_response = manager_client.get_flow_cell_positions().await?;

        if position_response.total_count > 0 {
            log::info!("MinKnow flowcell positions detected:");
            for position in &position_response.positions {
                log::info!("{}", position);
            }
        } else {
            log::info!("MinKnow flowcell positions: {}", &position_response);
        }

        let active_positions: HashMap<String, FlowCellPosition> = HashMap::from_iter(
            position_response.positions.iter().map(|p| (p.name.clone(), p.clone()))
        );
        
        // # Multiplexing requests [from Tonic]
        //
        // Sending a request on a channel requires a `&mut self` and thus can only send
        // one request in flight. This is intentional and is required to follow the `Service`
        // contract from the `tower` library which this channel implementation is built on
        // top of.
        //
        // `tower` itself has a concept of `poll_ready` which is the main mechanism to apply
        // back pressure. `poll_ready` takes a `&mut self` and when it returns `Poll::Ready`
        // we know the `Service` is able to accept only one request before we must `poll_ready`
        // again. Due to this fact any `async fn` that wants to poll for readiness and submit
        // the request must have a `&mut self` reference.
        //
        // To work around this and to ease the use of the channel, `Channel` provides a
        // `Clone` implementation that is _cheap_. This is because at the very top level
        // the channel is backed by a `tower_buffer::Buffer` which runs the connection
        // in a background task and provides a `mpsc` channel interface. Due to this
        // cloning the `Channel` type is cheap and encouraged.
        //
        // Performance wise we have found you can get a decent amount of throughput by just
        // clonling a single channel. Though the best comes when you load balance a few 
        // channels and also clone them around (https://github.com/hyperium/tonic/issues/285)

        // I may have to revisit this as the requests get more complex, e.g. on streaming 
        // acquisition data or sending off reads for basecalling.
    
        Ok(Self {
            tls: tls.clone(),
            config: config.clone(),
            clients: ActiveClients { manager: manager_client },
            channels: ActiveChannels { manager: manager_channel },
            positions: ActivePositions { positions: active_positions }
        })
    }
    // Opens a new channel to the requested position and logs the channel state stream to the terminal
    pub async fn stream_channel_states_log(&self, position_name: &str, first_channel: u32, last_channel: u32) -> Result<(), Box<dyn std::error::Error>> {

        // Opens a new secure channel to the requested position RPC port and 
        // issues the request to start streaming channel data

        let mut data_client = DataClient::from_minknow_client(&self, position_name).await?;

        let mut stream = data_client.stream_channel_states(
            first_channel,
            last_channel,
            None,
            false,
            Some(prost_types::Duration { seconds: 1, nanos: 0 })  // 0.1 seconds = 100000000 ns
         ).await?;

        // Logging the streamed data as formatted terminal output
        while let Some(state_response) = stream.message().await? {
            for channel_state in state_response.channel_states {
                log::info!("{}", channel_state) // might be inefficient
            }
        }
        Ok(())
    }
    // Opens a new channel to the requested position and logs the channel state stream into an async queue (one receiver, multiple sender) for testing
    pub async fn stream_channel_states_queue_log(&self, position_name: &str, first_channel: u32, last_channel: u32) -> Result<(), Box<dyn std::error::Error>> {

        // Opens a new secure channel to the requested position RPC port and 
        // issues the request to start streaming channel data

        let mut data_client = DataClient::from_minknow_client(&self, position_name).await?;

        let mut stream = data_client.stream_channel_states(
            first_channel,
            last_channel,
            None,
            false,
            Some(prost_types::Duration { seconds: 1, nanos: 0 })  // 0.1 seconds = 100000000 ns
         ).await?;


        // The channel will buffer up to the provided number of messages. Once the buffer is full, attempts to
        // send new messages will wait until a message is received from the channel. All data sent on Sender 
        // will become available on Receiver in the same order as it was sent. The Sender can be cloned to 
        // send to the same channel from multiple code locations. Only one Receiver is supported. If the 
        // Receiver is disconnected while trying to send, the send method will return a SendError. 
        // Similarly, if Sender is disconnected while trying to recv, the recv method will return None.
        
        let (state_tx, mut state_rx) = tokio::sync::mpsc::channel(1000);
        
        // Spawn an async thread that streams the responses from the RPC endpoint and sends each channel's state to the queue
        tokio::spawn(async move { 
            while let Some(state_response) = stream.message().await.expect("Could not get message from stream") {
                for state_data in state_response.channel_states {
                    state_tx.send(state_data).await.expect("Could not send message on channel")
                }
            }   
        });
        
        // Concurrently receive the state data and print their summary from the queue 
        while let Some(msg) = state_rx.recv().await {
            log::info!("{}", msg)
        }

        Ok(())
    }
    
}