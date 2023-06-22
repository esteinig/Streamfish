


use::colored::*;
use thiserror::Error;
use std::collections::HashMap;
use tonic::transport::{ClientTlsConfig, Certificate, Channel};

use crate::config::ReefsquidConfig;
use crate::client::data::DataClient;
use crate::client::manager::ManagerClient;
use crate::services::minknow_api::data::get_live_reads_response::ReadData;
use crate::{config::MinknowConfig, services::minknow_api::manager::FlowCellPosition};





#[derive(Error, Debug)]
pub enum ActivePositionError {
    /// Represents a failure to obtain the port of a position - this
    /// usually occurs when the position is not in a running state
    #[error("failed to obtain port of the requested position ({0})")]
    PortNotFound(String),
    /// Represents a failure to obtain a position - this
    /// likely due to that the position was not running at 
    /// initiation of the MinknowClient connection with the 
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
pub struct MinknowClient{
    pub tls: ClientTlsConfig,
    pub config: MinknowConfig,
    pub clients: ActiveClients,
    pub channels: ActiveChannels,
    pub positions: ActivePositions
}

impl MinknowClient {

    pub async fn connect(config: &MinknowConfig) -> Result<Self, Box<dyn std::error::Error>> {

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


use crate::services::minknow_api::data::get_live_reads_request::{Action, UnblockAction, StreamSetup, StopFurtherData, Request::Setup};
use crate::services::minknow_api::data::GetLiveReadsRequest;


pub struct ReadUntilClient {
    pub minknow: MinknowClient
}

impl ReadUntilClient {

    pub async fn new(config: &ReefsquidConfig) -> Result<Self, Box<dyn std::error::Error>> {

        let minknow_client = MinknowClient::connect(&config.minknow).await?;

        Ok(Self { minknow: minknow_client })
    }

    pub async fn run(&mut self, position_name: &str, channel_start: &u32, channel_end: &u32) -> Result<(), Box<dyn std::error::Error>> {

        let mut data_client = DataClient::from_minknow_client(
            &self.minknow, position_name
        ).await?;

        // ==============
        // Message queues
        // ==============

        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel(1024);
        let (basecall_tx, mut basecall_rx) = tokio::sync::mpsc::channel(1024);

        // ===========================
        // Streams from message queues
        // ===========================

        // Setup the action request stream from this client - this stream reveives 
        // messages from the action queue (`GetLiveReadsRequests`)
        let action_request_stream = async_stream::stream! {
            while let Some(action_request) = action_rx.recv().await {
                log::info!("Action message received => request stream");
                yield action_request;
            }
        };

        // Setup the basecall request stream from this client - this stream reveives 
        // messages from the basecall queue (`GetLiveReadsRequests`)
        let basecall_request_stream = async_stream::stream! {
            while let Some(basecall_request) = basecall_rx.recv().await {
                log::debug!("Basecall message received => request stream");
                yield basecall_request;
            }
        };

        // =========================
        // Data stream setup request
        // =========================

        // Setup the initial request to setup the data stream ...
        let init_action = GetLiveReadsRequest { request: Some(Setup(StreamSetup { 
                first_channel: *channel_start, 
                last_channel: *channel_end, 
                raw_data_type: 2, // 1 = daq | 2 = pico amp
                sample_minimum_chunk_size: 100,
                accepted_first_chunk_classifications: vec![83, 65], 
                max_unblock_read_length: None
            }))
        };

        // Send it into the action queue that unpacks into the request stream
        let init_action_stream_tx = action_tx.clone();
        init_action_stream_tx.send(init_action.clone()).await?;

        // Send it into the action queue that unpacks into the request stream
        // let init_basecall_stream_tx = basecall_tx.clone();
        // init_basecall_stream_tx.send(init_action.clone()).await?;


        // With this stream we now send the request to the server which returns a stream of responses that contain the data...
        let action_request = tonic::Request::new(action_request_stream);
        let mut data_stream = data_client.client.get_live_reads(action_request).await?.into_inner();
        
        let stop_action_tx = action_tx.clone(); // places message on the data action stream

        let action_stream_handle = tokio::spawn(async move {
            while let Some(response) = data_stream.message().await.expect("Failed to get message from stream") {
                
                for (channel, read_data) in response.channels {
                    log::info!("Channel {:<5} => {}", channel, read_data);
                }

                // When we receive the raw data we send an action to stop further reads that we want
                // to analyse into the action request stream and send a basecall request with
                // the acquisition data into the basecall request stream

                // Note that if the read has finished when the stop further data action is sent,
                // and error is returned for the action (need to match above in the loop)

                // let stop_further_data_action = GetLiveReadsRequest { request: None };
                // action_tx.send(stop_further_data_action).await.expect("Failed to send message into channel"); // 

                // let test = GetLiveReadsRequest { request: None };
                // basecall_tx.send(test).await.expect("Failed to send message into channel");
            }
        });



        // Just testing if this works...
        let action_request = tonic::Request::new(basecall_request_stream);
        let mut basecall_stream: tonic::Streaming<crate::services::minknow_api::data::GetLiveReadsResponse> = data_client.client.get_live_reads(action_request).await?.into_inner();
        
        let basecall_action_tx = action_tx.clone(); // places message on the data action stream

        let basecall_stream_handle = tokio::spawn(async move {
            while let Some(response) = basecall_stream.message().await.expect("Failed to get message from stream") {
                
                for (channel, read_data) in response.channels {
                    log::info!("BASECALL Channel {:<5} => {}", channel, read_data);
                }
            }
        });

        // Await thread handles to run the streams
        for handle in [
            action_stream_handle,
            basecall_stream_handle
        ] {
            handle.await?
        };

        Ok(())
    }
}


impl std::fmt::Display for ReadData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let id_short = &self.id[..8];
        let classification_str = format!("{}", self.previous_read_classification);
        let (pstrand, pid) = match self.previous_read_classification == 80 {
            true => (classification_str.bright_green(), id_short.bright_green()),
            false => match self.previous_read_classification == 83 {
                true => (classification_str.green(), id_short.green()), 
                false => (classification_str.white(), id_short.white())
            }
        };
        write!(
            f, "{:<3} {} {:?}", 
            pstrand, 
            pid,
            self.chunk_classifications
        )
    }
}

// Test function to transform one stream into another - see the for await syntax from the async_stream crate
// async fn get_read_response_stream<S: Stream<Item = Result<GetLiveReadsResponse, tonic::Status>>>(inbound: S, tx: tokio::sync::mpsc::Sender<u32>) -> impl Stream<Item = GetLiveReadsResponse> {
//     async_stream::stream! {
//         for await response in inbound {
//            let read_response = response.unwrap();

//             // Do some things during unpacking ofthis stream into another stream

//            yield read_response
//         }
//     }
// }
//
// let response_stream = get_read_response_stream(inbound, state_tx).await;
//  pin_mut!(response_stream);
//  while let Some(response) = response_stream.next().await {
//      log::info!("{:?}", response)
//  };

        