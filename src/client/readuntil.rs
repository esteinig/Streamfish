

use uuid::Uuid;
use::colored::*;

use crate::config::ReefsquidConfig;
use crate::client::minknow::MinKnowClient;
use crate::client::services::data::DataClient;
use crate::client::services::analysis::AnalysisConfigurationClient;

use crate::server::dori::DoriClient;
use crate::services::dori_api::basecaller::{DoradoBasecallerRequest, DoradoBasecallerResponse};
use crate::services::minknow_api::data::GetLiveReadsRequest;
use crate::services::minknow_api::data::get_live_reads_request::action;
use crate::services::minknow_api::data::get_live_reads_response::ReadData;
use crate::services::minknow_api::data::get_live_reads_request::{Actions, Action, StopFurtherData, UnblockAction, StreamSetup, Request}; 


pub struct ReadUntilClient {
    pub dori: DoriClient,
    pub minknow: MinKnowClient
}

impl ReadUntilClient {

    pub async fn connect(config: &ReefsquidConfig) -> Result<Self, Box<dyn std::error::Error>> {

        Ok(Self { 
            dori: DoriClient::connect(&config.dori).await?, 
            minknow: MinKnowClient::connect(&config.minknow).await? 
        })
    }

    pub async fn run(&mut self, position_name: &str, channel_start: &u32, channel_end: &u32, unblock_duration: f64, unblock_all: bool, unblock_dori: bool) -> Result<(), Box<dyn std::error::Error>> {

        // ==============================
        // MinKnow DataService connection
        // ==============================

        let mut data_client = DataClient::from_minknow_client(
            &self.minknow, position_name
        ).await?;

        let read_classifications = 
            AnalysisConfigurationClient::from_minknow_client(
                &self.minknow, position_name
            )
            .await?
            .get_read_classifications().await?;

        log::info!("{:#?}", read_classifications);

        // ==========================================
        // MPSC message queues: senders and receivers
        // ==========================================

        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel(1024);
        let (dori_tx, mut dori_rx) = tokio::sync::mpsc::channel(1024);

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
                raw_data_type: 3, // 1 = daq | 2 = pico amp
                sample_minimum_chunk_size: 100,
                accepted_first_chunk_classifications: vec![83, 65], 
                max_unblock_read_length: None
            }))
        };

        // Send it into the action queue that unpacks into the request stream
        let init_action_stream_tx = action_tx.clone();
        init_action_stream_tx.send(init_action.clone()).await?;

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

                    if unblock_all && !unblock_dori {

                        // Unblock all to test unblocking, equivalent to Readfish implementation
                        // do not send a request to the Dori::BasecallerService stream
                        data_action_tx.send(GetLiveReadsRequest { request: Some(
                            Request::Actions(Actions { actions: vec![
                                Action {
                                    action_id: Uuid::new_v4().to_string(),
                                    read: Some(action::Read::Number(read_data.number)),
                                    action: Some(action::Action::Unblock(UnblockAction { duration: unblock_duration })),
                                    channel: channel,
                                }
                            ]})
                        )}).await.expect("Failed to unblock request to queue");

                    } else {
                        // Otherwise always send the request to the basecaller request stream,
                        // where the unblock request is put in the queue from the response stream
                        dori_basecall_tx.send(DoradoBasecallerRequest {
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

                }
            }
        });


        // ========================================
        // DoriService response stream is processed
        // ========================================

        let dori_action_tx = action_tx.clone();

        let dori_stream_handle = tokio::spawn(async move {
            while let Some(dori_reponse) = dori_stream.message().await.expect("Failed to get message from stream") {

                log::info!("Channel {:<5} => {}", dori_reponse.channel, dori_reponse);

                if unblock_all && unblock_dori {
                    dori_action_tx.send(GetLiveReadsRequest { request: Some(
                        Request::Actions(Actions { actions: vec![
                            Action {
                                action_id: Uuid::new_v4().to_string(),
                                read: Some(action::Read::Number(dori_reponse.number)),
                                action: Some(action::Action::Unblock(UnblockAction { duration: unblock_duration })),
                                channel: dori_reponse.channel,
                            }
                        ]})
                    )}).await.expect("Failed to unblock request to queue");
                };
            }
        });

        // Await thread handles to run the streams
        for handle in [
            action_stream_handle,
            dori_stream_handle
        ] {
            handle.await?
        };

        Ok(())
    }
}


impl std::fmt::Display for DoradoBasecallerResponse {
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

        