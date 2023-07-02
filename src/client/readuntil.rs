use std::path::PathBuf;

use uuid::Uuid;
use::colored::*;
use quanta::{Clock, Instant};

use crate::config::{
    StreamfishConfig, 
    ReadUntilConfig
};

use crate::server::dori::DoriClient;
use crate::client::minknow::MinKnowClient;
use crate::client::services::data::DataClient;

use crate::services::dori_api::basecaller::basecaller_response::Decision;
use crate::services::minknow_api::data::GetLiveReadsRequest;
use crate::services::minknow_api::data::get_live_reads_request::action;
use crate::services::minknow_api::data::get_live_reads_response::ReadData;

use crate::services::dori_api::basecaller::{
    BasecallerRequest,
    BasecallerResponse
};
use crate::services::minknow_api::data::get_live_reads_request::{
    Actions, 
    Action, 
    StopFurtherData, 
    UnblockAction, 
    StreamSetup, 
    Request
}; 

use tokio::fs::File;
use tokio::io::AsyncWriteExt;


#[derive(Debug)]
enum PipelineStage {
    DoriRequest,
    DoriUnblock,
    MinKnowUnblock
}
impl PipelineStage {
    pub fn as_str_name(&self) -> &str {
        match self {
            PipelineStage::DoriRequest => "dori_request",
            PipelineStage::DoriUnblock => "dori_unblock",
            PipelineStage::MinKnowUnblock => "minknow_unblock",
        }
    }
}

#[derive(Debug)]
pub struct ClientLog {
    stage: PipelineStage,
    time: Instant,
    // For tracing purposes mainly
    read_id: Option<String>,
    // Using channel and read number identification as these are u32 instead of String
    // NOTE: read numbers always increment throughout the experiment, and are unique per 
    // channel - however they are not necessarily contiguous (i.e. can be used for ID, 
    // but not sequential inferences)
    channel: u32,
    number: u32
}
impl ClientLog {
    pub fn millis_since_start(&self, start: Instant) -> u128 {
        self.time.duration_since(start).as_millis()
    } 
    pub fn micros_since_start(&self, start: Instant) -> u128 {
        self.time.duration_since(start).as_micros()
    } 
    pub fn nanos_since_start(&self, start: Instant) -> u128 {
        self.time.duration_since(start).as_nanos()
    } 
    pub fn entry(&self, start: Instant) -> String {
        format!("{} {} {} {} {}\n", self.stage.as_str_name(), match &self.read_id { Some(id) => &id, None => "-" }, self.channel, self.number,  self.micros_since_start(start))
    }
}


#[derive(Debug)]
pub struct StatusLog {
    msg: String
}

pub struct ReadUntilClient {
    pub dori: DoriClient,
    pub minknow: MinKnowClient,
    pub readuntil: ReadUntilConfig,
}

// Do not use Strings when it can be avoided, introduces too much latency, use static strings (&str) or enumerations
// this introducted a bit of latency into the logging as string name conversion

impl ReadUntilClient {

    pub async fn connect(config: &mut StreamfishConfig, log_latency: &Option<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {

        // Some configurations can be set from the command-line
        // and are overwritten before connection of the clients
        if let Some(_) = log_latency {
            config.readuntil.log_latency = log_latency.clone()
        }

        if config.readuntil.unblock_all {
            log::warn!("Immediate unblocking of all reads is active!");
        }
        if config.readuntil.unblock_all_dori {
            log::warn!("Dori response unblocking of all reads is active!");
        }
        if config.readuntil.unblock_all_process {
            log::warn!("Dori process response unblocking of all reads is active!");
        }

        Ok(Self { 
            dori: DoriClient::connect(&config).await?, 
            minknow: MinKnowClient::connect(&config.minknow).await?,
            readuntil: config.readuntil.clone(),
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        let unblock_decision: i32 = Decision::Unblock.into();


        let run_config = self.readuntil.clone();
        let clock = Clock::new();

        // ==============================
        // MinKnow DataService connection
        // ==============================

        let mut data_client = DataClient::from_minknow_client(
            &self.minknow, &run_config.device_name
        ).await?;

        // log::info!("Read classifications {:#?}", read_classifications);

        // ==========================================
        // MPSC message queues: senders and receivers
        // ==========================================
        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel(4096);
        let (dori_tx, mut dori_rx) = tokio::sync::mpsc::channel(4096);
        let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(10000);

        // =========================================================
        // Message queue receivers unpack into async request streams
        // =========================================================

        // Setup the action request stream from this client - this stream reveives 
        // messages from the action queue (`GetLiveReadsRequests`)
        let data_request_stream = async_stream::stream! {
            while let Some(action_request) = action_rx.recv().await {
                yield action_request;
            }
        };

        // Setup the basecall request stream from this client - this stream reveives 
        // messages from the basecall queue (`GetLiveReadsRequests`)
        let dori_request_stream = async_stream::stream! {
            while let Some(dori_request) = dori_rx.recv().await {
                yield dori_request;
            }
        };

        // ==========================================
        // Request and response streams are initiated
        // ==========================================

        // Setup the initial request to setup the data stream ...
        let init_action = GetLiveReadsRequest { request: Some(Request::Setup(StreamSetup { 
                first_channel: run_config.channel_start, 
                last_channel: run_config.channel_end, 
                raw_data_type: run_config.raw_data_type.into(), 
                sample_minimum_chunk_size: run_config.sample_minimum_chunk_size,
                accepted_first_chunk_classifications: run_config.accepted_first_chunk_classifications, 
                max_unblock_read_length: None
            }))
        };

        // Send it into the action queue that unpacks into the request stream 
        // - this must happen before the request to MinKNOW
        let init_action_stream_tx = action_tx.clone();
        init_action_stream_tx.send(init_action.clone()).await?;
        log::info!("Initiated data streams with MinKNOW");

        // DataService response stream is initiated with the data request stream to MinKNOW
        let minknow_request = tonic::Request::new(data_request_stream);
        let mut minknow_stream = data_client.client.get_live_reads(minknow_request).await?.into_inner();

        // BasecallService response stream is initiated with the dori request stream
        let dori_request = tonic::Request::new(dori_request_stream);
        let mut dori_stream = self.dori.client.basecall_dorado(dori_request).await?.into_inner();

        log::info!("Initiated data streams with Dori");
        
        let start = clock.now();
        log::info!("Started streaming loop for adaptive sampling");

        // =================================================
        // MinKnow::DataService response stream is processed
        // =================================================

        let minknow_response_log = log_tx.clone();
        let minknow_response_clock = clock.clone();

        let minknow_action_tx = action_tx.clone(); // unpacks stop further data action requests into data request stream
        let dori_basecall_tx = dori_tx.clone(); // unpacks dori basecall action requests into dori request stream

        let action_stream_handle = tokio::spawn(async move {
            while let Some(response) = minknow_stream.message().await.expect("Failed to get response from Minknow data stream") {
                

                // Keep for logging action response states

                // for action_response in response.action_responses {
                //     if action_response.response != 0 {
                //         log::warn!("Failed action: {} ({})", action_response.action_id, action_response.response);
                //     }
                // }
                
                for (channel, read_data) in response.channels {

                    // See if it's worth to spawn threads?
                    
                    // log::info!("Channel {:<5} => {}", channel, read_data);
                    // log::info!("Chunk length: {}", read_data.chunk_length);

                    if run_config.unblock_all {
                        // Unblock all to test unblocking, equivalent to Readfish implementation
                        // do not send a request to the Dori::BasecallerService stream
                        minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                            Request::Actions(Actions { actions: vec![
                                Action {
                                    action_id: Uuid::new_v4().to_string(), 
                                    read: Some(action::Read::Number(read_data.number)),
                                    action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                                    channel: channel,
                                }
                            ]})
                        )}).await.expect("Failed to send unblock request to Minknow request queue");

                    } else {
                        // Otherwise always send the request to the basecaller request stream,
                        // where the unblock request is put in the queue from the response stream
                        dori_basecall_tx.send(BasecallerRequest {
                            id: read_data.id, 
                            channel: channel,
                            number: read_data.number,
                            data: read_data.raw_data,
                        }).await.expect("Failed to send basecall requests to Dori request queue")
                    }

                    // Always request to stop further data from the current read
                    // as we are currently not accumulating in caches
                    minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                        Request::Actions(Actions { actions: vec![
                            Action {
                                action_id: Uuid::new_v4().to_string(),
                                read: Some(action::Read::Number(read_data.number)),
                                action: Some(action::Action::StopFurtherData(StopFurtherData {})),
                                channel: channel,
                            }
                        ]})
                    )}).await.expect("Failed to send stop further data request to Minknow request queue"); 

                    minknow_response_log.send(ClientLog { 
                        stage: PipelineStage::DoriRequest, 
                        time: minknow_response_clock.now(), 
                        read_id: None,
                        channel: channel, 
                        number: read_data.number 
                    }).await.expect("Failed to send log message from Minknow response stream");

                }
            }
        });
        
        // ========================================
        // DoriService response stream is processed
        // ========================================

        let dori_response_log = log_tx.clone();
        let dori_response_clock = clock.clone();

        let minknow_dori_action_tx = action_tx.clone();

        let dori_stream_handle = tokio::spawn(async move {
            while let Some(dori_response) = dori_stream.message().await.expect("Failed to get response from Dori response stream") {

                // log::info!("Channel {:<5} => {}", &dori_response.channel, &dori_response);

                // Evaluate the Dori response - a response may be sent from different
                // stages of the basecall-classifier pipeline at the endpoint - for
                // now we will unblock at the second stage, after classifier output

                if dori_response.decision != unblock_decision {
                    continue;
                }

                minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                    Request::Actions(Actions { actions: vec![
                        Action {
                            action_id: Uuid::new_v4().to_string(),
                            read: Some(action::Read::Number(dori_response.number)),
                            action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                            channel: dori_response.channel,
                        }
                    ]})
                )}).await.expect("Failed to unblock request to queue");

                dori_response_log.send(ClientLog { 
                    stage: PipelineStage::DoriUnblock, 
                    read_id: Some(dori_response.id),
                    time: dori_response_clock.now(), 
                    channel: dori_response.channel, 
                    number: dori_response.number 
                }).await.expect("Failed to send log message from Dori response stream");
            }
        });

    
        let logging_handle = tokio::spawn(async move {

            // Routine when specifing a log file in configuration:
            if let Some(path) = run_config.log_latency {

                let mut log_file = File::create(&path).await.expect(
                    &format!("Failed to open log file {}", &path.display())
                );

                while let Some(log) = log_rx.recv().await {
                    log::info!("{} {} {}", log.stage.as_str_name(), log.channel, log.number);
                    log_file.write_all(log.entry(start).as_bytes()).await.expect("Failed to write entry to log file");
                }
                
            } else {
                if run_config.print_latency {
                    while let Some(log) = log_rx.recv().await {
                        println!("{}", log.entry(start))
                    }
                } else {
                    while let Some(log) = log_rx.recv().await {
                        log::info!("{} {} {}", log.stage.as_str_name(), log.channel, log.number);
                    }
                }
               
            }

            
        });

        // ===================================
        // Await thread handles to run streams
        // ===================================

        for handle in [
            action_stream_handle,
            dori_stream_handle,
            logging_handle
        ] {
            handle.await?
        };

        Ok(())

    }
}


impl std::fmt::Display for BasecallerResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let id_short = &self.id[..8];
        write!(
            f, "{} {}", 
            id_short.blue(),
            self.number
        )
    }
}

impl std::fmt::Display for ReadData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let id_short = &self.id[..8];
        let pid = match self.previous_read_classification {
            80 => id_short.bright_green(),
            83 => id_short.green(),
            _ => id_short.white()
        };
        write!(
            f, "{} {}", 
            pid,
            self.number
        )
    }
}
