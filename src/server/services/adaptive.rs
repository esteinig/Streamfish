
use crate::client::minknow::MinKnowClient;
use crate::config::StreamfishConfig;
use crate::services::dori_api::adaptive::{DoradoStreamRequest, DoradoStreamResponse};
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSampling;
use crate::services::dori_api::adaptive::{DoradoCacheRequest, DoradoCacheResponse, dorado_cache_response::Decision, dorado_cache_request::Request as DoradoCacheDataRequest};

use std::collections::HashMap;
use std::pin::Pin;
use futures_core::Stream;
use futures_util::StreamExt;
use futures_util::io::{BufReader, Lines};
use futures_util::AsyncWriteExt;
use futures_util::io::AsyncBufReadExt;
use tonic::{Request, Response, Status};
use async_process::{Command, Stdio, ChildStdout, ChildStdin};
use itertools::join;

pub struct AdaptiveSamplingService {
    config: StreamfishConfig
}
impl AdaptiveSamplingService {
    pub fn new(config: &StreamfishConfig) -> Self {

        // Some checks on server

        if !config.dori.basecaller_model_path.exists() {
            panic!("Basecaller model path does not exist")
        }
        if !config.dori.classifier_reference_path.exists() {
            panic!("Classifier reference/database path does not exist")
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
        
        log::info!("Basecaller: {} Path: {:?} Args: {:?}", config.dori.basecaller, config.dori.basecaller_path, config.dori.basecaller_args);
        log::info!("Classifier: {} Path: {:?} Args: {:?}", config.dori.classifier, config.dori.classifier_path, config.dori.classifier_args);

        Self { config: config.clone() }
    }
}


// Service implementation matches the RPC definitions translated during build - see the trait annotations on BasecallerService
#[tonic::async_trait]
impl AdaptiveSampling for AdaptiveSamplingService {

    type DoradoCacheStream = Pin<Box<dyn Stream<Item = Result<DoradoCacheResponse, Status>> + Send  + 'static>>;
    type DoradoStreamStream = Pin<Box<dyn Stream<Item = Result<DoradoStreamResponse, Status>> + Send  + 'static>>;

    // Adaptive sampling with Dorado basecalling and no request/read cache - single request/read processing only
    async fn dorado_stream(&self, _: Request<tonic::Streaming<DoradoStreamRequest>>) -> Result<Response<Self::DoradoStreamStream>, Status> {

        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoStream RPC");

        unimplemented!("No implemented yet")

    }

    // Adaptive sampling with Dorado basecalling and request/read cache implementation
    async fn dorado_cache(&self, request: Request<tonic::Streaming<DoradoCacheRequest>>) -> Result<Response<Self::DoradoCacheStream>, Status> {
        
        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoCache RPC");

        // Used in generators, needs distinct clones
        let run_config_1 = self.config.clone();
        let run_config_2 = self.config.clone();

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Actions sent to MinKNOW
        let stop_decision: i32 = Decision::Stop.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // No actions sent to MinKNOW
        let none_decision: i32 = Decision::None.into();     
        let continue_decision: i32 = Decision::Continue.into();

        // Connection to MinKNOW to obtain device calibration data
        let minknow_client = MinKnowClient::connect(
            &self.config.minknow
        ).await.expect("Failed to connect to MinKNOW");

        let (sample_rate, calibration) = minknow_client.get_device_data(
            &self.config.readuntil.device_name, 
            &self.config.readuntil.channel_start,  
            &self.config.readuntil.channel_end
        ).await.expect("Failed to get device calibration from MinKNOW");
        
        // Use channel specific caches due to not having to use a string value for lookup
        let mut channel_caches: Vec<HashMap<u32, Vec<DoradoCacheRequest>>> = Vec::new();

        for _ in 0..self.config.readuntil.channel_end {
            channel_caches.push(HashMap::new())
        }
        
        // ======================
        // Pipeline process setup
        // ======================

        let (mut pipeline_stdin, mut pipeline_stdout) = init_pipeline(&self.config);

        // =========================
        // Request stream processing
        // =========================

        let mut request_stream = request.into_inner();
     
        let pipeline_input_response_stream = async_stream::try_stream! {


            while let Some(dorado_request) = request_stream.next().await {
                let dorado_request = dorado_request?;


                let channel_index = (dorado_request.channel-1) as usize; // need to cast
                let read_cache = &mut channel_caches[channel_index];
                
                // Boolean value is just placeholer for `prost` syntax around `oneof`
                if let Some(DoradoCacheDataRequest::Uncache(_)) = dorado_request.request {
                    // If the request is a remove-read-from-cache request, do this
                    // before any further processing - this gets around the mutable
                    // borrow issue below and allows for other decisions to send
                    // cache removal requests later
                    read_cache.remove(&dorado_request.number);

                    let response = DoradoCacheResponse { 
                        channel: dorado_request.channel, 
                        number: dorado_request.number,
                        decision: none_decision
                    };
                    yield response

                } 

                // If request is not uncache or initialize, process the input data,
                // this cloning is annoying but neccessary for the cache operations

                let channel = dorado_request.channel.clone();
                let number = dorado_request.number.clone();

                // Find cached read or insert new one, update the cache with the current read
                read_cache.entry(dorado_request.number).or_insert(Vec::new()).push(dorado_request);

                // Get the updated read and the number of chunks in the cache
                let cached = read_cache.get(&number).expect("Failed to get data from channel cache");
                let num_chunks = cached.len();

                log::info!("Channel: {} Read: {} Chunks: {}", &channel, &number, &num_chunks);

                if num_chunks <= run_config_1.readuntil.read_cache_min_chunks {
                    // If we have less chunks than the minimum number required
                    // send a continue response for more data acquisition on 
                    // this read (no action)
                    let response = DoradoCacheResponse { 
                        channel, 
                        number,
                        decision: continue_decision
                    };
                    yield response

                } else if num_chunks >= run_config_1.readuntil.read_cache_max_chunks {
                    // If we have reached the maximum chunks in cache,
                    // do not process and send a stop response for
                    // ceasing data acquisition. This will also send
                    // a remove from cache request as we cannot do this
                    // below because ofthe required mutable borrow of the cache

                    // read_cache.remove(&number); not possible!

                    let response = DoradoCacheResponse { 
                        channel, 
                        number,
                        decision: stop_decision
                    };
                    yield response
                }

                // If we have an acceptable number of chunks between the limits, process 
                // the read by concatenating the chunks and sending it into the basecall
                // and alignment pipeline 
                
                let data: Vec<String> = cached.iter().map(|req| {
                    req.to_stdin(
                        calibration.offsets[channel_index],
                        calibration.pa_ranges[channel_index],
                        calibration.digitisation,
                        sample_rate
                    )
                }).collect();

                pipeline_stdin.write_all(data.concat().as_bytes()).await?;

                // Send a none decision response to take no action after writing into process
                // this response is mainly for logging 
                let response = DoradoCacheResponse { 
                    channel, 
                    number,
                    decision: none_decision
                };
                yield response
            }
        };
        

        // =========================
        // Process stream processing
        // =========================

        let test = true;
        let pipeline_output_response_stream = async_stream::try_stream! {

            while let Some(line) = pipeline_stdout.next().await {
                let line = line?;
                if line.starts_with('@') { 
                    continue; 
                } else {
                    match test {
                        true => yield cache_process_dorado_read_sam(&line, &run_config_2, unblock_decision, continue_decision).await,
                        false => yield cache_process_dorado_read_sam(&line, &run_config_2, unblock_decision, continue_decision).await
                    }
                }
            }
        };

        let response_stream_1 = futures::stream::select(
            pipeline_input_response_stream,
            pipeline_output_response_stream, 
        );

        Ok(Response::new(Box::pin(response_stream_1) as Self::DoradoCacheStream))
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
async fn cache_process_dorado_read_sam(line: &str, config: &StreamfishConfig, unblock_decision: i32, continue_decision: i32) -> DoradoCacheResponse {
                
        let content: Vec<&str> = line.split('\t').collect();
        let identifiers: Vec<&str> = content[0].split("::").collect();

        let flag = content[1].parse::<u32>().unwrap();
        log::info!("TID Dorado: {} {}", &flag, content[2]);

        let decision = match config.readuntil.unblock_all_process {
            true => unblock_decision,
            false => {
                match flag {
                    4 => continue_decision,     // unmapped
                    _ => unblock_decision,      // all other
                }
            }
        };

        DoradoCacheResponse { 
            channel: identifiers[1].parse::<u32>().unwrap(), 
            number: identifiers[2].parse::<u32>().unwrap(), 
            decision
        }
}

// Dorado output processing - handles SAM output format from aligned reads 
// and uses the configuration to set the unblock decision sent back to MinKNOW
//
// SAM specifictation for Dorado: https://github.com/nanoporetech/dorado/blob/master/documentation/SAM.md
// async fn stream_process_dorado_read_sam(line: &str, config: &StreamfishConfig, unblock_decision: i32, continue_decision: i32) -> DoradoStreamResponse {
                
//     let content: Vec<&str> = line.split('\t').collect();
//     let identifiers: Vec<&str> = content[0].split("::").collect();

//     let flag = content[1].parse::<u32>().unwrap();
//     log::info!("TID Dorado: {} {}", &flag, content[2]);

//     let decision = match config.readuntil.unblock_all_process {
//         true => unblock_decision,
//         false => {
//             match flag {
//                 4 => continue_decision,     // unmapped
//                 _ => unblock_decision,      // all other
//             }
//         }
//     };

//     DoradoStreamResponse { 
//         channel: identifiers[1].parse::<u32>().unwrap(), 
//         number: identifiers[2].parse::<u32>().unwrap(), 
//         decision
//     }
// }



use byteorder::{LittleEndian, ByteOrder};

impl DoradoCacheRequest {
    pub fn to_stdin(&self, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

        match &self.request {
            Some(DoradoCacheDataRequest::RawData(data)) => {
                // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
                let mut signal_data: Vec<i16> = Vec::new();
                for i in (0..data.len()).step_by(2) {
                    signal_data.push(LittleEndian::read_i16(&data[i..]));
                }

                // TODO CHECK EFFECTS: Float formatting precision - POD5 inspection suggest 13 digits for range (PYTHON) - not sure if necessary
                //
                // We are not using read identifiers, just the channel and numbers - this saves allocation of Strings but means we have to trace outputs by channel/number if 
                // for example cross reference with standard Guppy basecalls - we should not end up doing this because we will evaluate on Dorado calls from BLOW5/POD5 and can
                // modify the identifiers there.
                //
                // This adds a change to the Dorado input stream processing where we did this before by concatenating the channel::number to the read identifier
                // chich is now removed in commit: f08d3aed27818e44d1637f76e8b9cb00c7c79a6b on branch `dori-stdin-v0.3.1` - no changes to output parsing functions necessary
                format!("dori::{}::{} {} {} {} {:.1} {:.11} {} {}\n", self.channel, self.number, self.channel, self.number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
            },
            _ => panic!("Could not get raw data from request")
        }
    }
}

// impl DoradoStreamRequest {
//     pub fn to_stdin(&self, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

//         // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
//         let mut signal_data: Vec<i16> = Vec::new();
//         for i in (0..self.data.len()).step_by(2) {
//             signal_data.push(LittleEndian::read_i16(&self.data[i..]));
//         }

//         // TODO CHECK EFFECTS: Float formatting precision - POD5 inspection suggest 13 digits for range (PYTHON) - not sure if necessary
//         format!("{} {} {} {} {:.1} {:.11} {} {}\n", self.id, self.channel, self.number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
//     }
// }