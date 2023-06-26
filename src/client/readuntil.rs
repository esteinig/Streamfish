use uuid::Uuid;
use::colored::*;

use crate::config::{
    ReefsquidConfig, 
    ReadUntilConfig
};

use crate::server::dori::DoriClient;
use crate::client::minknow::MinKnowClient;
use crate::client::services::data::DataClient;
use crate::client::services::device::DeviceClient;
use crate::client::services::analysis::AnalysisConfigurationClient;

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

// Allows for multiple different log
// types to be sent to logging queue

#[derive(Debug)]
enum Log {
    Latency(LatencyLog),
    Status(StatusLog)
}
impl Log {
    pub fn as_str_name(&self) -> String {
        match self {
            Log::Latency(_) => "LatencyLog".to_string(),
            Log::Status(_) => "StatusLog".to_string()
        }
    }
}

#[derive(Debug)]
pub struct LatencyLog {

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

impl ReadUntilClient {

    pub async fn connect(config: &ReefsquidConfig) -> Result<Self, Box<dyn std::error::Error>> {

        Ok(Self { 
            dori: DoriClient::connect(&config.dori).await?, 
            minknow: MinKnowClient::connect(&config.minknow).await?,
            readuntil: config.readuntil.clone(),
        })
    }

    pub async fn run(
        &mut self, 
        position_name: &str, 
        channel_start: &u32, 
        channel_end: &u32, 
    ) -> Result<(), Box<dyn std::error::Error>> {

        let run_config = self.readuntil.clone();

        // ==============================
        // MinKnow DataService connection
        // ==============================

        let mut data_client = DataClient::from_minknow_client(
            &self.minknow, position_name
        ).await?;

        let mut device_client = DeviceClient::from_minknow_client(
            &self.minknow, position_name
        ).await?;

        let read_classifications = 
            AnalysisConfigurationClient::from_minknow_client(
                &self.minknow, position_name
            )
            .await?
            .get_read_classifications().await?;

        let sample_rate = device_client.get_sample_rate().await?;
        let calibration = device_client.get_calibration(channel_start, channel_end).await?;

        log::info!("Sample rate: {} | Digitisation: {}", sample_rate, calibration.digitisation);
        log::info!("Read classifications {:#?}", read_classifications);

        // ==========================================
        // MPSC message queues: senders and receivers
        // ==========================================
        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel(2048);
        let (dori_tx, mut dori_rx) = tokio::sync::mpsc::channel(2048);
        let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(4096);

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
                first_channel: *channel_start, 
                last_channel: *channel_end, 
                raw_data_type: run_config.raw_data_type.into(), 
                sample_minimum_chunk_size: run_config.sample_minimum_chunk_size,
                accepted_first_chunk_classifications: run_config.accepted_first_chunk_classifications, 
                max_unblock_read_length: None
            }))
        };


        // Send it into the action queue that unpacks into the request stream
        let init_action_stream_tx = action_tx.clone();
        init_action_stream_tx.send(init_action.clone()).await?;

        // Data stream initiation log
        log_tx.send(Log::Status(StatusLog { msg: "Initiated data stream from MinKNOW".into() })).await?;

        // DataService response stream is initiated with the data request stream
        let action_request = tonic::Request::new(data_request_stream);
        let mut data_stream = data_client.client.get_live_reads(action_request).await?.into_inner();


        // BasecallService response stream is initiated with the dori request stream
        let dori_request = tonic::Request::new(dori_request_stream);
        let mut dori_stream = self.dori.client.basecall_dorado(dori_request).await?.into_inner();
        

        // =================================================
        // MinKnow::DataService response stream is processed
        // =================================================

        let data_action_tx = action_tx.clone(); // unpacks stop further data action requests into data request stream
        let dori_basecall_tx = dori_tx.clone(); // unpacks dori basecall action requests into dori request stream

        let action_stream_handle = tokio::spawn(async move {
            while let Some(response) = data_stream.message().await.expect("Failed to get message from stream") {
                
                // Keep for logging action response states

                // for action_response in response.action_responses {
                //     if action_response.response != 0 {
                //         log::warn!("Failed action: {} ({})", action_response.action_id, action_response.response);
                //     }
                // }

                for (channel, read_data) in response.channels {
                    
                    log::info!("Channel {:<5} => {}", channel, read_data);

                    if run_config.unblock_all && !run_config.unblock_dori {
                        // Unblock all to test unblocking, equivalent to Readfish implementation
                        // do not send a request to the Dori::BasecallerService stream
                        data_action_tx.send(GetLiveReadsRequest { request: Some(
                            Request::Actions(Actions { actions: vec![
                                Action {
                                    action_id: Uuid::new_v4().to_string(),
                                    read: Some(action::Read::Number(read_data.number)),
                                    action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                                    channel: channel,
                                }
                            ]})
                        )}).await.expect("Failed to unblock request to queue");

                    } else {
                        // Otherwise always send the request to the basecaller request stream,
                        // where the unblock request is put in the queue from the response stream
                        dori_basecall_tx.send(BasecallerRequest {
                            id: read_data.id, 
                            channel: channel,
                            number: read_data.number,
                            data: read_data.raw_data,
                        }).await.expect("Failed to send basecall requests to queue")
                    }

                    // Always request to stop further data from the current read
                    data_action_tx.send(GetLiveReadsRequest { request: Some(
                        Request::Actions(Actions { actions: vec![
                            Action {
                                action_id: Uuid::new_v4().to_string(),
                                read: Some(action::Read::Number(read_data.number)),
                                action: Some(action::Action::StopFurtherData(StopFurtherData {})),
                                channel: channel,
                            }
                        ]})
                    )}).await.expect("Failed to send stop further data request to queue"); 

                    log_tx.send(Log::Status(StatusLog { msg: "Sent read data to Dori".into() })).await.expect("Failed to send log message from action stream");

                }
            }
        });
        
        // ========================================
        // DoriService response stream is processed
        // ========================================

        let dori_action_tx = action_tx.clone();

        let dori_stream_handle = tokio::spawn(async move {
            while let Some(dori_response) = dori_stream.message().await.expect("Failed to get message from stream") {

                log::info!("Channel {:<5} => {}", dori_response.channel, dori_response);

                // Evaluate the Dori response - a response may be sent from different
                // stages of the basecall-classifier pipeline at the endpoint - for
                // now we will unblock at the second stage, after classifier output

                if dori_response.stage != 2 { // PipelineStage::ClassifierOutput
                    continue;
                }

                dori_action_tx.send(GetLiveReadsRequest { request: Some(
                    Request::Actions(Actions { actions: vec![
                        Action {
                            action_id: Uuid::new_v4().to_string(),
                            read: Some(action::Read::Number(dori_response.number)),
                            action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                            channel: dori_response.channel,
                        }
                    ]})
                )}).await.expect("Failed to unblock request to queue");
            }
        });

        // Adding a logging thread seems increase latency by quite a bit? 10-15 bp

        // Not sure if the queue size is responsible? Notable blocking of actions
        // after around 20m - not terrible, but introduces stream loop lag and 
        // additional small peak at ~400 bp during unblock testing

        // Test if the block is less pronounced when spawning a task within the 
        // main task? This seems to work very well, might apply to other sections
        // test if it was queue size that influenced it.

        // Queue size it was most likely. I also think latency may be introduced
        // when the Rust compiler runs on all threads in the background, might be
        // hard to check through
    
        let logging_hande = tokio::spawn(async move {
            while let Some(log) = log_rx.recv().await {
                log::info!("Log received: {}", log.as_str_name());

                // Consider spawning long runing tasks here, e.g. writing to file:
                
                // tokio::spawn(async move {
                //     log::info!("Log received: {}", log.as_str_name());
                // });
            }
        });

        // ===================================
        // Await thread handles to run streams
        // ===================================

        for handle in [
            action_stream_handle,
            dori_stream_handle,
            logging_hande
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
