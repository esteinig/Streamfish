use std::char::MAX;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
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
use tokio::io::AsyncWriteExt as TokioAsyncWriteExt;


use futures_util::StreamExt;
use futures_util::AsyncWriteExt;
use futures_util::io::{BufReader, Lines};
use futures_util::io::AsyncBufReadExt;
use async_process::{Command, Stdio, ChildStdout, ChildStdin};
use itertools::join;


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
    pub minknow: MinKnowClient,
    pub config: StreamfishConfig,
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
            minknow: MinKnowClient::connect(&config.minknow).await?,
            config: config.clone(),
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        let run_config = self.config.readuntil.clone();

        let clock = Clock::new();


        let (sample_rate, calibration) = self.minknow.get_device_data(
            &run_config.device_name, 
            &run_config.channel_start,  
            &run_config.channel_end
        ).await.expect("Failed to get device calibration from MinKNOW");

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

        // ==========================================
        // Request and response streams are initiated
        // ==========================================

        // Setup the initial request to setup the data stream ...
        let init_action = GetLiveReadsRequest { request: Some(Request::Setup(StreamSetup { 
                first_channel: run_config.channel_start, 
                last_channel: run_config.channel_end, 
                raw_data_type: run_config.raw_data_type.into(), 
                sample_minimum_chunk_size: run_config.sample_minimum_chunk_size,
                accepted_first_chunk_classifications: run_config.accepted_first_chunk_classifications.clone(), 
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


        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        let none: i32 = Decision::None.into();
        let pass: i32 = Decision::Pass.into();
        let unblock: i32 = Decision::Unblock.into();

        // ======================
        // Pipeline process setup
        // ======================

        let (mut pipeline_stdin, mut pipeline_stdout) = init_pipeline(&self.config);

       
        let start = clock.now();
        log::info!("Started streaming loop for adaptive sampling");

        // =================================================
        // MinKnow::DataService response stream is processed
        // =================================================

        let minknow_response_log = log_tx.clone();
        let minknow_response_clock = clock.clone();

        let minknow_action_tx = action_tx.clone(); // unpacks stop further data action requests into dat

        let action_stream_handle = tokio::spawn(async move {


            while let Some(response) = minknow_stream.message().await.expect("Failed to get response from Minknow data stream") {
                

                // Keep for logging action response states

                // for action_response in response.action_responses {
                //     if action_response.response != 0 {
                //         log::warn!("Failed action: {} ({})", action_response.action_id, action_response.response);
                //     } else {
                //         log::info!("Action response: {:?}", action_response)
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
                        let channel_index = (channel-1) as usize;

                        // Use this to generate a test dataset to pipe into Dorado STDIN

                        // println!("{}", dorado_request.to_stdin(
                        //     calibration.offsets[channel_index],
                        //     calibration.pa_ranges[channel_index],
                        //     calibration.digitisation,
                        //     sample_rate
                        // ).trim());

                        // Writing as block of reads not in channel loop does not decrease 
                        // latency in tests, but may need to be revisited
                        
                        pipeline_stdin.write_all(
                            to_stdin(
                                &read_data.id,
                                &read_data.raw_data,
                                channel,
                                read_data.number,
                                calibration.offsets[channel_index],
                                calibration.pa_ranges[channel_index],
                                calibration.digitisation,
                                sample_rate
                            ).as_bytes()
                        ).await.expect("Failed to write read data to pipeline");
                        
                        

                    }

                    // // Always request to stop further data from the current read
                    // // as we are currently not accumulating in caches
                    // minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                    //     Request::Actions(Actions { actions: vec![
                    //         Action {
                    //             action_id: Uuid::new_v4().to_string(),
                    //             read: Some(action::Read::Number(read_data.number)),
                    //             action: Some(action::Action::StopFurtherData(StopFurtherData {})),
                    //             channel: channel,
                    //         }
                    //     ]})
                    // )}).await.expect("Failed to send stop further data request to Minknow request queue"); 

                    // minknow_response_log.send(ClientLog { 
                    //     stage: PipelineStage::DoriRequest, 
                    //     time: minknow_response_clock.now(), 
                    //     read_id: None,
                    //     channel: channel, 
                    //     number: read_data.number 
                    // }).await.expect("Failed to send log message from Minknow response stream");

                }
            }
        });
        
        // ========================================
        // DoriService response stream is processed
        // ========================================

        let dori_response_log = log_tx.clone();
        let dori_response_clock = clock.clone();

        let minknow_dori_action_tx = action_tx.clone();
        
        
        let test = true; // testing conditional processing
        
        let dori_stream_handle = tokio::spawn(async move {
            
            while let Some(line) = pipeline_stdout.next().await {
                let line = line.expect("Failed to parse line");

                if line.starts_with('@') { 
                    continue; 
                } 

                let (channel, number, decision) = match test {
                    true => process_dorado_read_sam(&line, run_config.unblock_all_process, unblock, pass).await,
                    false => process_dorado_read_sam(&line, run_config.unblock_all_process, unblock, pass).await
                };

                if decision != unblock {
                    continue;
                }


                // An unblock is sent to MinKNOW
                minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                    Request::Actions(Actions { actions: vec![
                        Action {
                            action_id: Uuid::new_v4().to_string(),
                            read: Some(action::Read::Number(number)),
                            action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                            channel: channel,
                        }
                    ]})
                )}).await.expect("Failed to unblock request to queue");

                // Log this action

                dori_response_log.send(ClientLog { 
                    stage: PipelineStage::DoriUnblock, 
                    read_id: None,
                    time: dori_response_clock.now(), 
                    channel: channel, 
                    number: number 
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



// Initiates the process pipeline from configuration
fn init_pipeline(config: &StreamfishConfig) -> (ChildStdin, Lines<BufReader<ChildStdout>>) {

    let process_stderr = Stdio::from(std::fs::File::create("basecaller.err").expect("Failed to create basecaller stderr file: basecaller.err"));

    let mut pipeline_process = Command::new(config.dori.basecaller_path.as_os_str())
        .args(config.dori.basecaller_args.clone())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(process_stderr)
        .spawn()
        .expect("Failed to spawn basecaller process");

    (
        pipeline_process.stdin.take().expect("Failed to open basecaller STDIN"),
        BufReader::new(
            pipeline_process.stdout.take().expect("Failed to open basecaller STDOUT")
        ).lines()
    )
}



// Dorado output processing - handles SAM output format from aligned reads 
// and uses the configuration to set the unblock decision sent back to MinKNOW
//
// SAM specifictation for Dorado: https://github.com/nanoporetech/dorado/blob/master/documentation/SAM.md
async fn process_dorado_read_sam(line: &str, unblock_all_process: bool, unblock_decision: i32, pass_decision: i32) -> (u32, u32, i32) {
                
        let content: Vec<&str> = line.split('\t').collect();
        let identifiers: Vec<&str> = content[0].split("::").collect();

        let flag = content[1].parse::<u32>().unwrap();
        
        let decision = match unblock_all_process {
            true => unblock_decision,
            false => {
                match flag {
                    4 => unblock_decision,     // unmapped 
                    _ => pass_decision,        // mapped continue
                }
            }
        };
        (identifiers[1].parse::<u32>().unwrap(), identifiers[2].parse::<u32>().unwrap(), decision)
}


use byteorder::{LittleEndian, ByteOrder};

pub fn to_stdin(id: &String, raw_data: &Vec<u8>, channel: u32, number: u32, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

    // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
    let mut signal_data: Vec<i16> = Vec::new();
    for i in (0..raw_data.len()).step_by(2) {
        signal_data.push(LittleEndian::read_i16(&raw_data[i..]));
    }

    // TODO CHECK EFFECTS: Float formatting precision - POD5 inspection suggest 13 digits for range (PYTHON) - not sure if necessary
    format!("{} {} {} {} {:.1} {:.11} {} {}\n", id, channel, number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
}


// Total desaster on streams to use a locking shared cache - blocks the streams of course

// const MAX_CACHED_CHUNKS: usize = 3;
// const MIN_CACHED_CHUNKS: usize = 0;

// let with_cache = true;

// let read_cache: Arc<Mutex<hashbrown::HashMap<String, Vec<Vec<u8>>>>> = Arc::new(Mutex::new(hashbrown::HashMap::new()));

// let arch_cache1 = Arc::clone(&read_cache);
// let action_stream_handle = tokio::spawn(async move {


//     while let Some(response) = minknow_stream.message().await.expect("Failed to get response from Minknow data stream") {
        

//         // Keep for logging action response states

//         // for action_response in response.action_responses {
//         //     if action_response.response != 0 {
//         //         log::warn!("Failed action: {} ({})", action_response.action_id, action_response.response);
//         //     } else {
//         //         log::info!("Action response: {:?}", action_response)
//         //     }
//         // }
        
//         for (channel, read_data) in response.channels {

//             // See if it's worth to spawn threads?
            
//             // log::info!("Channel {:<5} => {}", channel, read_data);
//             // log::info!("Chunk length: {}", read_data.chunk_length);

//             if run_config.unblock_all {
//                 // Unblock all to test unblocking, equivalent to Readfish implementation
//                 // do not send a request to the Dori::BasecallerService stream
//                 minknow_action_tx.send(GetLiveReadsRequest { request: Some(
//                     Request::Actions(Actions { actions: vec![
//                         Action {
//                             action_id: Uuid::new_v4().to_string(), 
//                             read: Some(action::Read::Number(read_data.number)),
//                             action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
//                             channel: channel,
//                         }
//                     ]})
//                 )}).await.expect("Failed to send unblock request to Minknow request queue");

//             } else {
//                 let channel_index = (channel-1) as usize;

//                 // Use this to generate a test dataset to pipe into Dorado STDIN

//                 // println!("{}", dorado_request.to_stdin(
//                 //     calibration.offsets[channel_index],
//                 //     calibration.pa_ranges[channel_index],
//                 //     calibration.digitisation,
//                 //     sample_rate
//                 // ).trim());

//                 // Writing as block of reads not in channel loop does not decrease 
//                 // latency in tests, but may need to be revisited
                

//                 // Look up the read cache if we have seen this read already - the 
//                 // lookup itself adds around 5 bp N50 in an unblock-all through analyis
//                 // (without testing the actual caching since reads are rejected
//                 // after the first iteration)
                
//                 if with_cache {

//                     let mut cache = arch_cache1.lock().await;

//                     cache.entry_ref(&read_data.id).or_insert(Vec::new()).push(read_data.raw_data);
//                     let cached_data = cache.get(&read_data.id).expect("Could not find cached read_data");

//                     let num_chunks = cached_data.len();

//                     if num_chunks >= MAX_CACHED_CHUNKS {
//                         // If we have reached the limit of chunks for analysis remove
//                         // and continue
//                         cache.remove(&read_data.id);
//                         continue
//                     } else if num_chunks <= MIN_CACHED_CHUNKS {
//                         continue                            
//                     } else if num_chunks == 1 {

//                         pipeline_stdin.write_all(
//                             to_stdin(
//                                 &read_data.id,
//                                 &cached_data[0],
//                                 channel,
//                                 read_data.number,
//                                 calibration.offsets[channel_index],
//                                 calibration.pa_ranges[channel_index],
//                                 calibration.digitisation,
//                                 sample_rate
//                             ).as_bytes()
//                         ).await.expect("Failed to write read data to pipeline");
                        
//                     } else {

                    
//                         pipeline_stdin.write_all(
//                             to_stdin(
//                                 &read_data.id,
//                                 &cached_data.concat(),
//                                 channel,
//                                 read_data.number,
//                                 calibration.offsets[channel_index],
//                                 calibration.pa_ranges[channel_index],
//                                 calibration.digitisation,
//                                 sample_rate
//                             ).as_bytes()
//                         ).await.expect("Failed to write read data to pipeline");

//                     }
//                 } else {
//                     pipeline_stdin.write_all(
//                         to_stdin(
//                             &read_data.id,
//                             &read_data.raw_data,
//                             channel,
//                             read_data.number,
//                             calibration.offsets[channel_index],
//                             calibration.pa_ranges[channel_index],
//                             calibration.digitisation,
//                             sample_rate
//                         ).as_bytes()
//                     ).await.expect("Failed to write read data to pipeline");
//                 }
                

//             }

//             // // Always request to stop further data from the current read
//             // // as we are currently not accumulating in caches
//             // minknow_action_tx.send(GetLiveReadsRequest { request: Some(
//             //     Request::Actions(Actions { actions: vec![
//             //         Action {
//             //             action_id: Uuid::new_v4().to_string(),
//             //             read: Some(action::Read::Number(read_data.number)),
//             //             action: Some(action::Action::StopFurtherData(StopFurtherData {})),
//             //             channel: channel,
//             //         }
//             //     ]})
//             // )}).await.expect("Failed to send stop further data request to Minknow request queue"); 

//             // minknow_response_log.send(ClientLog { 
//             //     stage: PipelineStage::DoriRequest, 
//             //     time: minknow_response_clock.now(), 
//             //     read_id: None,
//             //     channel: channel, 
//             //     number: read_data.number 
//             // }).await.expect("Failed to send log message from Minknow response stream");

//         }
//     }
// });

// // ========================================
// // DoriService response stream is processed
// // ========================================

// let dori_response_log = log_tx.clone();
// let dori_response_clock = clock.clone();

// let minknow_dori_action_tx = action_tx.clone();


// let test = true; // testing conditional processing

// let arch_cache2 = Arc::clone(&read_cache);
// let dori_stream_handle = tokio::spawn(async move {
    
//     while let Some(line) = pipeline_stdout.next().await {
//         let line = line.expect("Failed to parse line");

//         if line.starts_with('@') { 
//             continue; 
//         } 

//         let (id, channel, number, decision) = match test {
//             true => process_dorado_read_sam(&line, run_config.unblock_all_process, unblock, pass).await,
//             false => process_dorado_read_sam(&line, run_config.unblock_all_process, unblock, pass).await
//         };

//         if decision != unblock {
//             // No message sent to MinKNOW
//             continue;
//         }


//         // An unblock is sent to MinKNO and we remove this read from the cache
//         minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
//             Request::Actions(Actions { actions: vec![
//                 Action {
//                     action_id: Uuid::new_v4().to_string(),
//                     read: Some(action::Read::Number(number)),
//                     action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
//                     channel: channel,
//                 }
//             ]})
//         )}).await.expect("Failed to unblock request to queue");

//         let mut cache = arch_cache2.lock().await;
//         cache.remove(id);

//         // Log this action

//         dori_response_log.send(ClientLog { 
//             stage: PipelineStage::DoriUnblock, 
//             read_id: None,
//             time: dori_response_clock.now(), 
//             channel: channel, 
//             number: number 
//         }).await.expect("Failed to send log message from Dori response stream");
//     }
// });


// Look up the read cache if we have seen this read already - the 
// lookup itself adds around 5 bp N50 in an unblock-all through analyis
// (without testing the actual caching since reads are rejected
// after the first iteration)

// Maybe a cache is possible if the read is basecalled and then stored after basecalling for alignment and decision? Not sure how it would reach 
// the concurrent implementation again, unless the read cache is on Dori and another request with cache removal is sent to Dori endpoint.