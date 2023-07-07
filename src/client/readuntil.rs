use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;
use quanta::{
    Clock, 
    Instant
};

use crate::config::{
    StreamfishConfig, 
    ReadUntilConfig, ExperimentConfig
};

use crate::server::dori::DoriClient;
use crate::client::minknow::MinKnowClient;
use crate::client::services::data::DataClient;

use crate::services::dori_api::adaptive::{
    DoradoCacheRequestType, 
    DoradoCacheChannelRequest
};

use crate::services::dori_api::adaptive::Decision;
use crate::client::services::acquisition::AcquisitionClient;
use crate::services::minknow_api::data::GetLiveReadsRequest;
use crate::services::minknow_api::acquisition::MinknowStatus;
use crate::services::dori_api::adaptive::DoradoCacheBatchRequest;
use crate::services::minknow_api::data::get_live_reads_request::action;

use crate::services::minknow_api::data::get_live_reads_request::{
    Actions, 
    Action, 
    StopFurtherData, 
    UnblockAction, 
    StreamSetup, 
    Request as LiveReadsRequest
}; 

use tokio::fs::File;
use tokio::io::AsyncWriteExt;


#[derive(Debug)]
enum PipelineStage {
    DoriRequest,
    DoriResponse,
    MinKnowUnblock
}
impl PipelineStage {
    pub fn as_str_name(&self) -> &str {
        match self {
            PipelineStage::DoriRequest => "dori_request",
            PipelineStage::DoriResponse => "dori_response",
            PipelineStage::MinKnowUnblock => "minknow_unblock",
        }
    }
}

#[derive(Debug)]
pub struct ClientLog {
    stage: PipelineStage,
    time: Instant,
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
        format!("{} {} {} {}\n", self.stage.as_str_name(), self.channel, self.number,  self.micros_since_start(start))
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
    pub experiment: ExperimentConfig,
}

// Do not use Strings when it can be avoided, introduces too much latency, use str refs (&str) or enumerations
// this introducted a bit of latency into the logging as string name conversion

impl ReadUntilClient {

    pub async fn connect(config: &mut StreamfishConfig, log_latency: &Option<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {

        // Some configurations can be set from the command-line
        // and are overwritten before connection of the clients
        if let Some(_) = log_latency {
            config.readuntil.log_latency = log_latency.clone()
        }

        if config.readuntil.unblock_all_client {
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
            experiment: config.experiment.clone()
        })
    }

    pub async fn run_dorado_cache_batch(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Actions sent to MinKNOW
        let stop_decision: i32 = Decision::StopData.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // No actions sent to MinKNOW
        let _: i32 = Decision::None.into();     
        let _: i32 = Decision::Proceed.into();

        // Request type to Dori
        let data_request: i32 = DoradoCacheRequestType::Data.into();
        let init_request: i32 = DoradoCacheRequestType::Init.into();
        let cache_request: i32 = DoradoCacheRequestType::Cache.into();


        let run_config = self.readuntil.clone();
        let clock = Clock::new();

        // ==============================
        // MinKnow DataService connection
        // ==============================

        let mut data_client = DataClient::from_minknow_client(
            &self.minknow, &run_config.device_name
        ).await?;
        

        let mut acquisition_client = AcquisitionClient::from_minknow_client(
            &self.minknow, &run_config.device_name
        ).await?;

        // ==========================================
        // MPSC message queues: senders and receivers
        // ==========================================

        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel(run_config.action_stream_queue_buffer);
        let (dori_tx, mut dori_rx) = tokio::sync::mpsc::channel(run_config.dori_stream_queue_buffer);
        let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(run_config.logging_queue_buffer);

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


        // =================
        // Preflight checks
        // =================

        // Check experiment status for starting data acquisition/sequencing
        log::info!("Checking experiment status until sequencing commences");
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            interval.tick().await;
            let response = acquisition_client.get_current_status().await?;
            if response.status() == MinknowStatus::Processing {
                log::info!("Device started processing data...");  // must be processing not starting for request stream init
                break;
            } else {
                log::info!("Device status is: {}", response.status().as_str_name())
            }
        }

        // ==========================================
        // Request and response streams are initiated
        // ==========================================

        // Setup the initial request to setup the data stream ...
        let init_action = GetLiveReadsRequest { request: Some(LiveReadsRequest::Setup(StreamSetup { 
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
        action_tx.send(init_action).await?;
        log::info!("Initiated data streams with MinKNOW");

        // DataService response stream is initiated with the data request stream to MinKNOW
        let minknow_request = tonic::Request::new(data_request_stream);
        let mut minknow_stream = data_client.client.get_live_reads(minknow_request).await?.into_inner();

        // AdaptiveSamplingService response stream is initiated with the data request stream for the DoradoCache implementation on Dori
        let dori_request = tonic::Request::new(dori_request_stream);
        let mut dori_stream = self.dori.client.dorado_cache_batch(dori_request).await?.into_inner();

        // Setup clocks and queue clones for request and response streams
        let minknow_response_log = log_tx.clone();
        let minknow_response_clock = clock.clone();

        let minknow_action_tx = action_tx.clone(); 
        let dori_data_tx = dori_tx.clone(); 

        let dori_response_log = log_tx.clone();
        let dori_response_clock = clock.clone();

        let dori_action_tx = dori_tx.clone();
        let minknow_dori_action_tx = action_tx.clone();

        // Initiate the Dori request-response stream with the initiation request - this causes the pipeline process (basecaller/classifier)
        // to initiate and wait for data, including loading basecall models and databases, so there is no delay when data starts streaming
        dori_tx.send(DoradoCacheBatchRequest { channel: 0, number: 0, channels: HashMap::new(), request: init_request }).await?;
        log::info!("Initiated data streams with Dori");
        
        log::info!("Waiting {} seconds for pipeline initialisation on Dori...", &run_config.init_delay);
        tokio::time::sleep(tokio::time::Duration::new(run_config.init_delay, 0)).await;
        
        let start = clock.now();
        log::info!("Started streaming loop for adaptive sampling");

        // =================================================
        // MinKnow::DataService response stream is processed
        // =================================================

        let action_stream_handle = tokio::spawn(async move {
            while let Some(response) = minknow_stream.message().await.expect("Failed to get response from Minknow data stream") {
                
                if run_config.unblock_all_client {

                    for (channel, read_data) in response.channels {
                        // Unblock all to test unblocking, equivalent to Readfish
                        minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                            LiveReadsRequest::Actions(Actions { actions: vec![
                                Action {
                                    action_id: Uuid::new_v4().to_string(), // Check if this is really costly?
                                    read: Some(action::Read::Number(read_data.number)),
                                    action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                                    channel: channel,
                                }
                            ]})
                        )}).await.expect("Failed to send unblock request to Minknow request queue");

                        minknow_response_log.send(ClientLog { 
                            stage: PipelineStage::DoriRequest, 
                            time: minknow_response_clock.now(),
                            channel: channel, 
                            number: read_data.number 
                        }).await.expect("Failed to send log message from Minknow response stream");
                    }

                } else {
                
                    // Sends full channel data over to Dori - evaluate if single
                    // RPC requests per channel are not inferior in metrics - however
                    // when minimum chunk size for the cache is set higher the arrays
                    // get larger for transfer and it might impact latency - it seems to 
                    // choke sometimes but the distribution at 10 chunks is a solid 1.8kb N50
                    // seems more sensitive to system load though - other processses running 
                    // wil lchoke the stream more than individual requests sent.
                    dori_data_tx.send(DoradoCacheBatchRequest {
                        channels: response.channels,
                        channel: 0,
                        number: 0,
                        request: data_request
                    }).await.expect("Failed to send basecall requests to Dori request queue");

                    minknow_response_log.send(ClientLog { 
                        stage: PipelineStage::DoriRequest, 
                        time: minknow_response_clock.now(),
                        channel: 0, 
                        number: 0
                    }).await.expect("Failed to send log message from Minknow response stream");
                }
            }
        });
        
        // ========================================
        // DoriService response stream is processed
        // ========================================


        let dori_stream_handle = tokio::spawn(async move {
            while let Some(dori_response) = dori_stream.message().await.expect("Failed to parse response from Dori response stream") {

                // log::info!("Channel {:<5} => {}", &dori_response.channel, &dori_response);

                if  dori_response.decision == unblock_decision {

                    log::info!("Sending an unblock decision: {} {}", &dori_response.channel, &dori_response.number);
                        // Send unblock decision to stop read - also stops further data (minknow_api::data)
                        minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                            LiveReadsRequest::Actions(Actions { actions: vec![
                                Action {
                                    action_id: Uuid::new_v4().to_string(),
                                    read: Some(action::Read::Number(dori_response.number)),
                                    action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                                    channel: dori_response.channel,
                                }
                            ]})
                        )}).await.expect("Failed to unblock request to queue");

                        // Send uncache request to Dori to remove read from cache
                        dori_action_tx.send(DoradoCacheBatchRequest {
                            channel: dori_response.channel,
                            number: dori_response.number,
                            request: cache_request,
                            channels: HashMap::new()
                        }).await.expect("Failed to send basecall requests to Dori request queue")

                } else if dori_response.decision == stop_decision {

                    log::info!("Sending a stop further data decision: {} {}", &dori_response.channel, &dori_response.number);

                    // Send a stop receive further data action and let read be 
                    // sequenced without further evaluations
                    minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                        LiveReadsRequest::Actions(Actions { actions: vec![
                            Action {
                                action_id: Uuid::new_v4().to_string(),
                                read: Some(action::Read::Number(dori_response.number)),
                                action: Some(action::Action::StopFurtherData(StopFurtherData {})),
                                channel: dori_response.channel,
                            }
                        ]})
                    )}).await.expect("Failed to send stop further data request to Minknow request queue"); 

                    // Send uncache request to Dori to remove read from cache
                    dori_action_tx.send(DoradoCacheBatchRequest {
                        channel: dori_response.channel,
                        number: dori_response.number,
                        request: cache_request,
                        channels: HashMap::new()
                    }).await.expect("Failed to send basecall requests to Dori request queue")

                } else {
                    // Continue or none decisions are not processed, we let the client fetch
                    // more chunks from the read to be added to cache and only log
                }

                // Always send a log entry to queue
                dori_response_log.send(ClientLog { 
                    stage: PipelineStage::DoriResponse, 
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


    pub async fn run_dorado_cache_channel(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Actions sent to MinKNOW
        let stop_decision: i32 = Decision::StopData.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // No actions sent to MinKNOW
        let _: i32 = Decision::None.into();     
        let _: i32 = Decision::Proceed.into();

        // Request type to Dori
        let data_request: i32 = DoradoCacheRequestType::Data.into();
        let init_request: i32 = DoradoCacheRequestType::Init.into();
        let cache_request: i32 = DoradoCacheRequestType::Cache.into();


        let run_config = self.readuntil.clone();
        let clock = Clock::new();

        // ==============================
        // MinKnow DataService connection
        // ==============================

        let mut data_client = DataClient::from_minknow_client(
            &self.minknow, &run_config.device_name
        ).await?;


        let mut acquisition_client = AcquisitionClient::from_minknow_client(
            &self.minknow, &run_config.device_name
        ).await?;

        // ==========================================
        // MPSC message queues: senders and receivers
        // ==========================================

        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel(run_config.action_stream_queue_buffer);
        let (dori_tx, mut dori_rx) = tokio::sync::mpsc::channel(run_config.dori_stream_queue_buffer);
        let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(run_config.logging_queue_buffer);

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


        // =================
        // Preflight checks
        // =================

        // Check experiment status for starting data acquisition/sequencing
        log::info!("Checking experiment status until sequencing commences");
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        loop {
            interval.tick().await;
            let response = acquisition_client.get_current_status().await?;
            if response.status() == MinknowStatus::Processing {
                log::info!("Device started processing data...");  // must be processing not starting for request stream init
                break;
            } else {
                log::info!("Device status is: {}", response.status().as_str_name())
            }
        }

        // ==========================================
        // Request and response streams are initiated
        // ==========================================

        // Setup the initial request to setup the data stream ...
        let init_action = GetLiveReadsRequest { request: Some(LiveReadsRequest::Setup(StreamSetup { 
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
        action_tx.send(init_action).await?;
        log::info!("Initiated data streams with MinKNOW");

        // DataService response stream is initiated with the data request stream to MinKNOW
        let minknow_request = tonic::Request::new(data_request_stream);
        let mut minknow_stream = data_client.client.get_live_reads(minknow_request).await?.into_inner();

        // AdaptiveSamplingService response stream is initiated with the data request stream for the DoradoCache implementation on Dori
        let dori_request = tonic::Request::new(dori_request_stream);
        let mut dori_stream = self.dori.client.dorado_cache_channel(dori_request).await?.into_inner();

        // Setup clocks and queue clones for request and response streams
        let minknow_response_log = log_tx.clone();
        let minknow_response_clock = clock.clone();

        let minknow_action_tx = action_tx.clone(); 
        let dori_data_tx = dori_tx.clone(); 

        let dori_response_log = log_tx.clone();
        let dori_response_clock = clock.clone();

        let dori_action_tx = dori_tx.clone();
        let minknow_dori_action_tx = action_tx.clone();


        // Initiate Dori stream
        dori_tx.send(DoradoCacheChannelRequest { channel: 0, number: 0, data: Vec::new(), request: init_request }).await?;
        log::info!("Initiated data streams with Dori");

        log::info!("Waiting {} seconds for pipeline initialisation on Dori...", &run_config.init_delay);
        tokio::time::sleep(tokio::time::Duration::new(run_config.init_delay, 0)).await;
        

        let start = clock.now();
        log::info!("Started adaptive sampling loop");

        // =================================================
        // MinKnow::DataService response stream is processed
        // =================================================

        let action_stream_handle = tokio::spawn(async move {
            while let Some(response) = minknow_stream.message().await.expect("Failed to get response from Minknow data stream") {
                
                log::info!("Number of channels received: {}", response.channels.len());

                for (channel, read_data) in response.channels {
                    
                    if run_config.unblock_all_client {
                            // Unblock all to test unblocking, equivalent to Readfish
                            minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                                LiveReadsRequest::Actions(Actions { actions: vec![
                                    Action {
                                        action_id: Uuid::new_v4().to_string(), // Check if this is really costly?
                                        read: Some(action::Read::Number(read_data.number)),
                                        action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                                        channel: channel,
                                    }
                                ]})
                            )}).await.expect("Failed to send unblock request to Minknow request queue");
                    } else {
                    
                        // Sends single channel data to Dori - this was the initial implementation
                        // and may not be inferior to the batched request implementation above
                        dori_data_tx.send(DoradoCacheChannelRequest {
                            channel: channel,
                            number: read_data.number,
                            data: read_data.raw_data,
                            request: data_request
                        }).await.expect("Failed to send basecall requests to Dori request queue");
                        
                    }

                    minknow_response_log.send(ClientLog { 
                        stage: PipelineStage::DoriRequest, 
                        time: minknow_response_clock.now(),
                        channel: channel, 
                        number: read_data.number 
                    }).await.expect("Failed to send log message from Minknow response stream");
                }
            }
        });
        
        // ========================================
        // DoriService response stream is processed
        // ========================================


        let dori_stream_handle = tokio::spawn(async move {
            while let Some(dori_response) = dori_stream.message().await.expect("Failed to parse response from Dori response stream") {

                // log::info!("Channel {:<5} => {}", &dori_response.channel, &dori_response);

                if  dori_response.decision == unblock_decision {

                    log::info!("Sending an unblock decision: {} {}", &dori_response.channel, &dori_response.number);
                        // Send unblock decision to stop read - also stops further data (minknow_api::data)
                        minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                            LiveReadsRequest::Actions(Actions { actions: vec![
                                Action {
                                    action_id: Uuid::new_v4().to_string(),
                                    read: Some(action::Read::Number(dori_response.number)),
                                    action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                                    channel: dori_response.channel,
                                }
                            ]})
                        )}).await.expect("Failed to unblock request to queue");

                        // Send uncache request to Dori to remove read from cache
                        dori_action_tx.send(DoradoCacheChannelRequest {
                            channel: dori_response.channel,
                            number: dori_response.number,
                            request: cache_request,
                            data: Vec::new()
                        }).await.expect("Failed to send basecall requests to Dori request queue")

                } else if dori_response.decision == stop_decision {

                    log::info!("Sending a stop further data decision: {} {}", &dori_response.channel, &dori_response.number);

                    // Send a stop receive further data action and let read be 
                    // sequenced without further evaluations
                    minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                        LiveReadsRequest::Actions(Actions { actions: vec![
                            Action {
                                action_id: Uuid::new_v4().to_string(),
                                read: Some(action::Read::Number(dori_response.number)),
                                action: Some(action::Action::StopFurtherData(StopFurtherData {})),
                                channel: dori_response.channel,
                            }
                        ]})
                    )}).await.expect("Failed to send stop further data request to Minknow request queue"); 

                    // Send uncache request to Dori to remove read from cache
                    dori_action_tx.send(DoradoCacheChannelRequest {
                        channel: dori_response.channel,
                        number: dori_response.number,
                        request: cache_request,
                        data: Vec::new()
                    }).await.expect("Failed to send basecall requests to Dori request queue")

                } else {
                    // Continue or none decisions are not processed, we let the client fetch
                    // more chunks from the read to be added to cache and only log
                }

                // Always send a log entry to queue
                dori_response_log.send(ClientLog { 
                    stage: PipelineStage::DoriResponse, 
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

