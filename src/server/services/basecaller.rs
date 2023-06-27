
use crate::client::minknow::MinKnowClient;
use crate::client::services::device::DeviceClient;
use crate::config::StreamfishConfig;
use crate::services::dori_api::basecaller::basecaller_server::Basecaller;
use crate::services::dori_api::basecaller::{BasecallerRequest, BasecallerResponse, basecaller_response::PipelineStage};

use std::pin::Pin;
// use std::io::{Read, BufRead, BufReader, Write};
use futures_core::Stream;
use futures_util::StreamExt;
use futures_util::io::BufReader;
use futures_util::AsyncWriteExt;
use futures_util::io::AsyncBufReadExt;
use tonic::{Request as TonicRequest, Response as TonicResponse, Status as TonicStatus};
// use std::process::{Command, Stdio};
use async_process::{Command, Stdio};
use itertools::join;


use uuid::Uuid;
use::colored::*;
use quanta::Instant;

use crate::client::services::data::DataClient;

use crate::services::minknow_api::data::GetLiveReadsRequest;
use crate::services::minknow_api::data::get_live_reads_request::action;
use crate::services::minknow_api::data::get_live_reads_response::ReadData;

use crate::services::minknow_api::data::get_live_reads_request::{
    Actions, 
    Action, 
    StopFurtherData, 
    UnblockAction, 
    StreamSetup, 
    Request as MinknowDataRequest
}; 

// Allows for multiple different log
// types to be sent to logging queue

#[derive(Debug)]
enum Log {
    Latency(LatencyLog),
    Status(StatusLog)
}
impl Log {
    pub fn as_str_name(&self) -> &str {
        match self {
            Log::Latency(_) => "log_latency",
            Log::Status(_) => "log_status"
        }
    }
}



#[derive(Debug)]
pub struct LatencyLog {
    stage: PipelineStage,
    time: Instant,
    // Using channel and read number identification as these are u32 instead of String
    // NOTE: read numbers always increment throughout the experiment, and are unique per 
    // channel - however they are not necessarily contiguous (i.e. can be used for ID, 
    // but not sequential inferences)
    channel: u32,
    number: u32
}
impl LatencyLog {
    pub fn as_micros(&self, start: Instant) -> u128 {
        self.time.duration_since(start).as_micros()
    } 
}

#[derive(Debug)]
pub struct StatusLog {
    msg: String
}


pub struct BasecallerService {
    config: StreamfishConfig
}
impl BasecallerService {
    pub fn new(config: &StreamfishConfig) -> Self {

        Self { config: config.clone() }
    }
}

// Service implementation matches the RPC definitions translated during build - see the trait annotations on BasecallerService
#[tonic::async_trait]
impl Basecaller for BasecallerService {

    type BasecallDoradoStream = Pin<Box<dyn Stream<Item = Result<BasecallerResponse, TonicStatus>> + Send  + 'static>>;

    async fn basecall_dorado( 
        &self,
        _: TonicRequest<BasecallerRequest>,
    ) -> Result<TonicResponse<Self::BasecallDoradoStream>, TonicStatus> {
        
        let run_config = self.config.clone();

        // Ok so my understanding now is that input and output stream have to be linked - the request
        // stream must unpack into either a queue sending into the response stream from an async thread
        // or through unpacking responses directly into an async response stream, and merging multiple.
        // streams if there are multiple processing stages that happen concurrently (see below).
        // It seems that if these conditions are not met, e.g. when sending input stream into a 
        // background process and unpacking the background process outputs into a response stream, 
        // there is no streaming connection established i.e. when the initial request stream is not
        // also sending responses into the response stream.
        
        // I may be wrong though - my mistake yesterday was to await the async threads spawned, which never
        // finished because of the internal while loops that unpacked the input stream.

        // I think for now, the  setup is working really nicely with minimal latency and can be extended to
        // multiple pipes of background processing - just need to send response qualifiers (which part of the
        // server-side pipeline is sending the response) so that unblock actions are not triggered with
        // responses that return from e.g. basecaller input stage.


        // Asynchroneous basecaller process spawned as background task - is read within a spawned thread -
        // we use the async-process crate because it allows us to use async unpacking of outputs into a
        // response stream i.e. a loop over lines.next().await unpacking into async_stream::try_stream! 


        log::info!("Initiated Dori::BasecallerService::BasecallDorado on request stream connection");

        let minknow_client = MinKnowClient::connect(&run_config.minknow).await.expect("Failed to connect to MinKNOW from Dori");

        let mut device_client = DeviceClient::from_minknow_client(
            &minknow_client, &run_config.readuntil.device_name
        ).await.expect("Failed to connect to initiate device client on Dori");

        let sample_rate = device_client.get_sample_rate().await.expect("Failed to get sample rate from MinKNOW on Dori");
        let calibration = device_client.get_calibration(&run_config.readuntil.channel_start, &run_config.readuntil.channel_end).await.expect("Failed to connect to get calibration from MinKNOW on Dori");

        log::info!("Sample rate: {} | Digitisation: {}", sample_rate, calibration.digitisation);

        // Ok does this mean we can actually get the whole stream from here??

        let mut data_client = DataClient::from_minknow_client(
            &minknow_client, &run_config.readuntil.device_name
        ).await.expect("Failed to initialize data client on Dori");


        // Basecall stage

        let mut basecaller_process = Command::new(run_config.dori.dorado_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn basecaller process");

        let basecaller_stdin = std::sync::Arc::new(tokio::sync::Mutex::new(basecaller_process.stdin.take().expect("Failed to open basecaller STDIN")));
        let basecaller_stdout = basecaller_process.stdout.take().expect("Failed to open basecaller STDOUT");

        let mut basecaller_lines = BufReader::new(basecaller_stdout).lines();

        // Classification stage - at the moment just the same outputs with the minimal basecaller
        // input/output program to measure effects on latency

        let mut classifier_process = Command::new(run_config.dori.dorado_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn basecaller process");

        let classifier_stdin = std::sync::Arc::new(tokio::sync::Mutex::new(classifier_process.stdin.take().expect("Failed to open basecaller STDIN")));
        let classifier_stdout = classifier_process.stdout.take().expect("Failed to open basecaller STDOUT");

        let mut classifier_lines = BufReader::new(classifier_stdout).lines();


        let (action_tx, mut action_rx) = tokio::sync::mpsc::channel(2048);

        // Setup the action request stream from this client - this stream reveives 
        // messages from the action queue (`GetLiveReadsRequests`)
        let data_request_stream = async_stream::stream! {
            while let Some(action_request) = action_rx.recv().await {
                yield action_request;
            }
        };
        
        // Setup the initial request to setup the data stream ...
        let init_action = GetLiveReadsRequest { request: Some(MinknowDataRequest::Setup(StreamSetup { 
            first_channel: run_config.readuntil.channel_start, 
            last_channel: run_config.readuntil.channel_end, 
            raw_data_type: run_config.readuntil.raw_data_type.into(), 
            sample_minimum_chunk_size: run_config.readuntil.sample_minimum_chunk_size,
            accepted_first_chunk_classifications: run_config.readuntil.accepted_first_chunk_classifications.clone(), 
            max_unblock_read_length: None
            }))
        };

        // Send it into the action queue that unpacks into the request stream 
        // - this must happen before the request to MinKNOW
        let init_action_stream_tx = action_tx.clone();
        init_action_stream_tx.send(init_action.clone()).await.expect("Failed to send data stream intitiation to MinKNOW from Dori");


        // DataService response stream is initiated with the data request stream to MinKNOW
        let minknow_request = tonic::Request::new(data_request_stream);
        let mut minknow_stream = data_client.client.get_live_reads(minknow_request).await?.into_inner();


        let stop_read_data_tx = action_tx.clone();

        let unblock_dori = run_config.readuntil.unblock_dori;
        let unblock_all = run_config.readuntil.unblock_all;
        let unblock_duration = run_config.readuntil.unblock_duration;

        let minknow_data_stream = async_stream::try_stream! {

            while let Some(response) = minknow_stream.message().await.expect("Failed to get response from Minknow data stream on Dori") {
                    

                    // Keep for logging action response states
    
                    // for action_response in response.action_responses {
                    //     if action_response.response != 0 {
                    //         log::warn!("Failed action: {} ({})", action_response.action_id, action_response.response);
                    //     }
                    // }

                let stdin = std::sync::Arc::clone(&basecaller_stdin); // Clone once write multiple times
                let minknow_action_tx = stop_read_data_tx.clone(); // Hmmmm...

                tokio::spawn(async move {

                    let mut inner = stdin.lock().await;

                    for (channel, read_data) in response.channels {
    
                        // See if it's worth to spawn async threads?
                        
                        log::info!("Channel {:<5} => {}", channel, read_data);
    
                        if unblock_all && !unblock_dori {
                            // Unblock all to test unblocking, equivalent to Readfish implementation
                            // do not send a request to the Dori::BasecallerService stream
                            minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                                MinknowDataRequest::Actions(Actions { actions: vec![
                                    Action {
                                        action_id: Uuid::new_v4().to_string(),  // do the action ids matter?
                                        read: Some(action::Read::Number(read_data.number)),
                                        action: Some(action::Action::Unblock(UnblockAction { duration: unblock_duration })),
                                        channel: channel,
                                    }
                                ]})
                            )}).await.expect("Failed to send unblock action to queue");
    
                        }
    
                        minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                            MinknowDataRequest::Actions(Actions { actions: vec![
                                Action {
                                    action_id: Uuid::new_v4().to_string(),
                                    read: Some(action::Read::Number(read_data.number)),
                                    action: Some(action::Action::StopFurtherData(StopFurtherData {})),
                                    channel: channel,
                                }
                            ]})
                        )}).await.expect("Failed to send stop reads action to queue");

                        inner.write_all(read_data.to_stdin(&channel, &calibration.digitisation, &0).as_bytes()).await.expect("Failed to write data to basecaller input");
                    }

                });
                
                yield BasecallerResponse { 
                    channel: 0, 
                    number: 0, 
                    id: "minknow".to_string(),
                    stage: PipelineStage::MinknowDataResponse.into()
                }
            }

        };
        
        let basecaller_output_classifier_input_response_stream = async_stream::try_stream! {
            while let Some(line) = basecaller_lines.next().await {
                let line = line?;
                
                let stdin = std::sync::Arc::clone(&classifier_stdin);
                tokio::spawn(async move {
                    // Repeating the basecaller test input/output for now
                    let mut inner = stdin.lock().await;  // return is stripped
                    inner.write_all(line.as_bytes()).await.expect("Failed to write data to classifier input");
                    inner.write_all("\n".as_bytes()).await.expect("Failed to write data to classifier input");
                });

                yield BasecallerResponse { 
                    channel: 0, 
                    number: 0, 
                    id: "basecaller".to_string(),
                    stage: PipelineStage::BasecallerOutput.into()
                };
            }
        };

        let unblock_action_tx = action_tx.clone();

        let classifier_output_response_stream = async_stream::try_stream! {
            
            while let Some(line) = classifier_lines.next().await {
                let line = line?;

                // Here we unpack the output of the C++ JSON parser which
                // currently stands-in for Dorado and for latency testing - 
                // important because we want the channel and read number in 
                // channel to send back a potential unblock request
                let content: Vec<&str> = line.trim().split_whitespace().collect();

                let channel = content[1].parse::<u32>().unwrap();
                let number = content[2].parse::<u32>().unwrap();

                unblock_action_tx.send(GetLiveReadsRequest { request: Some(
                    MinknowDataRequest::Actions(Actions { actions: vec![
                        Action {
                            action_id: Uuid::new_v4().to_string(),
                            read: Some(action::Read::Number(number)),
                            action: Some(action::Action::Unblock(UnblockAction { duration: unblock_duration })),
                            channel: channel,
                        }
                    ]})
                )}).await.expect("Failed to unblock request to queue");

                yield BasecallerResponse { 
                    channel, 
                    number, 
                    id: "classifier".to_string(),
                    stage: PipelineStage::ClassifierOutput.into()
                };
            }
        };

        // Merge output streams - this producees no guaranteed order on stage responses

        // For some reason `futures::stream::select_all` does not work - complains about mismatch
        // between the result async stream type, showing the same type?
        //
        // Might need to report this to async_stream crate, this looks a bit inefficient
        let response_stream_1 = futures::stream::select(
            minknow_data_stream, 
            basecaller_output_classifier_input_response_stream,
        );
        let response_stream_2 = futures::stream::select(
            response_stream_1,
            classifier_output_response_stream,
        );

        Ok(TonicResponse::new(Box::pin(response_stream_2) as Self::BasecallDoradoStream))
    }

}

impl ReadData {
    pub fn to_stdin(&self, channel: &u32, digitisation: &u32, offset: &u32) -> String {
        format!("{} {} {} {} {} {}\n", self.id, channel, self.number, digitisation, offset, join(&self.raw_data, " "))
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
