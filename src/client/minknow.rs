


use thiserror::Error;
use std::collections::HashMap;
use crate::config::IcarustConfig;
use crate::client::error::ClientError;
use crate::client::services::data::DataClient;
use crate::client::services::manager::ManagerClient;
use tonic::transport::{ClientTlsConfig, Certificate, Channel};
use crate::client::services::device::{DeviceClient, DeviceCalibration};
use crate::{config::MinknowConfig, services::minknow_api::manager::FlowCellPosition};


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

#[derive(Debug, Clone)]
pub struct ActiveChannels {
    pub manager: Channel
}

#[derive(Debug, Clone)]
pub struct ActiveClients {
    pub manager: ManagerClient
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
// Main client for the MinKnow API
pub struct MinknowClient{
    pub tls: ClientTlsConfig,
    pub config: MinknowConfig,
    pub icarust: IcarustConfig,
    pub clients: ActiveClients,
    pub channels: ActiveChannels,
    pub positions: ActivePositions
}

impl MinknowClient {

    pub async fn connect(config: &MinknowConfig, icarust: &IcarustConfig) -> Result<Self, ClientError> {

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
        // We need to modify the ClientTlsConfig in `tonic` to disable certificate verifiction, using
        // this modified version of `tonic` which is available as a forkon my GitHub 
        //
        //      https://github.com/esteinig/tonic @ v0.9.2-r1

        let cert = std::fs::read_to_string(&config.certificate).expect(
            &format!("Failed to read certificate from path: {}", &config.certificate.display()) 
        );
        let tls = ClientTlsConfig::new()
            .domain_name("localhost")
            .ca_certificate(Certificate::from_pem(cert));

        // ========================
        // ManagerClient Initiation 
        // ========================

        log::info!("Connecting to control server on: https://{}:{}", config.host, config.port);

        // Establish a secure channel to the MinKnow manager service, that will be 
        // available throughout to request data from the manager service

        let manager_channel = Channel::from_shared(
            format!("https://{}:{}", config.host, config.port)
        ).map_err(|_| ClientError::InvalidUri)?
         .tls_config(tls.clone())
         .map_err( |_| ClientError::InvalidTls)?
         .connect().await
         .map_err(|_| ClientError::ControlServerConnectionInitiation)?;

        // Use a simple interceptor for authentication - this might have to be generalised
        // using `tower` middleware - but it's too complex for my simple brain right now.

        log::info!("Channel established on: https://{}:{}", config.host, config.port);
        
        let mut manager_client = ManagerClient::new(
            manager_channel.clone(), config.token.clone()
        );

        log::info!("Connected to control server manager service");

        // Get the version information to test the connection and print the version of MinKnow
        let version_response = manager_client.get_version_info().await?;
        log::info!("Control server version: v{}", version_response);

        // Get the flowcell positions and print their total count, if present, print their summary
        let position_response = manager_client.get_flow_cell_positions().await?;

        if position_response.total_count > 0 {
            log::info!("Flowcell positions detected:");
            for position in &position_response.positions {
                log::info!("{}", position);
            }
        } else {
            log::info!("Flowcell positions: {}", &position_response);
        }

        let active_positions: HashMap<String, FlowCellPosition> = HashMap::from_iter(
            position_response.positions.iter().map(|p| (p.name.clone(), p.clone()))
        );
            
        Ok(Self {
            tls: tls.clone(),
            config: config.clone(),
            icarust: icarust.clone(),
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

        let mut data_client = DataClient::from_minknow_client(&self, position_name).await?;

        let mut stream = data_client.stream_channel_states(
            first_channel,
            last_channel,
            None,
            false,
            Some(prost_types::Duration { seconds: 1, nanos: 0 })  // 0.1 seconds = 100000000 ns
         ).await?;
        
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
    // Get sample rate and calibration for a device
    pub async fn get_device_data(&self, position_name: &str, first_channel: &u32, last_channel: &u32) -> Result<(u32, DeviceCalibration), Box<dyn std::error::Error>> {

        let mut device_client = DeviceClient::from_minknow_client(&self, position_name).await?;

        // Need to account for the sample rate not being available for Icarust
        let sample_rate = match self.icarust.enabled {
            true => self.icarust.sample_rate,
            false => device_client.get_sample_rate().await?
        };

        let calibration = device_client.get_calibration(first_channel, last_channel).await?;

        Ok((sample_rate, calibration))
    }
    
}