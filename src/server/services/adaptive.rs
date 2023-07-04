
use crate::client::minknow::MinKnowClient;
use crate::config::StreamfishConfig;
use crate::services::dori_api::adaptive::{DoradoStreamRequest, DoradoStreamResponse};
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSampling;
use crate::services::dori_api::adaptive::{
    DoradoCacheBatchRequest, 
    DoradoCacheChannelRequest, 
    DoradoCacheResponse, 
    dorado_cache_response::Decision, 
    DoradoCacheRequestType
};
use crate::client::services::device::DeviceCalibration;

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
use async_stream::__private::AsyncStream;

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

    type DoradoCacheBatchStream = Pin<Box<dyn Stream<Item = Result<DoradoCacheResponse, Status>> + Send  + 'static>>;
    type DoradoCacheChannelStream = Pin<Box<dyn Stream<Item = Result<DoradoCacheResponse, Status>> + Send  + 'static>>;
    type DoradoStreamStream = Pin<Box<dyn Stream<Item = Result<DoradoStreamResponse, Status>> + Send  + 'static>>;

    // Adaptive sampling with Dorado basecalling and no request/read cache - single request/read processing only
    async fn dorado_stream(&self, _: Request<tonic::Streaming<DoradoStreamRequest>>) -> Result<Response<Self::DoradoStreamStream>, Status> {

        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoStream RPC");

        unimplemented!("No implemented yet")

    }

    // Adaptive sampling with Dorado basecalling and no request/read cache - single request/read processing only
    async fn dorado_cache_channel(&self, _: Request<tonic::Streaming<DoradoCacheChannelRequest>>) -> Result<Response<Self::DoradoCacheChannelStream>, Status> {

        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoStream RPC");

        unimplemented!("No implemented yet")

    }

    // Adaptive sampling with Dorado basecalling and request/read cache implementation
    async fn dorado_cache_batch(&self, request: Request<tonic::Streaming<DoradoCacheBatchRequest>>) -> Result<Response<Self::DoradoCacheBatchStream>, Status> {
        
        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoCache RPC");

        // Used in generators, needs distinct clones
        let run_config_2 = self.config.clone();

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Action sent to MinKNOW
        let stop_decision: i32 = Decision::Stop.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // No action sent to MinKNOW 
        let continue_decision: i32 = Decision::Continue.into();

        // Request types
        let init_request: i32 = DoradoCacheRequestType::Init.into();
        let cache_request: i32 = DoradoCacheRequestType::Cache.into();

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
        let mut channel_caches: Vec<HashMap<u32, Vec<Vec<u8>>>> = Vec::new();

        for _ in 0..self.config.readuntil.channel_end {
            channel_caches.push(HashMap::new())
        }
        
        // ======================
        // Pipeline process setup
        // ======================

        let (pipeline_stdin, mut pipeline_stdout) = init_pipeline(&self.config);

        // =========================
        // Request stream processing
        // =========================
     
        let request_response_stream = request_stream_cache_batch(
            request, channel_caches, pipeline_stdin, 
            init_request, cache_request, stop_decision, 
            self.config.clone(), calibration, sample_rate
        );
        

        // =========================
        // Process stream processing
        // =========================

        let test = true;
        let pipeline_response_stream = async_stream::try_stream! {

            while let Some(line) = pipeline_stdout.next().await {
                let line = line?;

                // log::info!("Reading line output from pipeline");
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


        // =========================
        // Response stream merge
        // =========================

        let response_stream = futures::stream::select(
            request_response_stream,
            pipeline_response_stream, 
        );

        Ok(Response::new(Box::pin(response_stream) as Self::DoradoCacheBatchStream))
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

pub fn get_dorado_input_string(raw_data: Vec<u8>, channel: u32, number: u32, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

    // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
    let mut signal_data: Vec<i16> = Vec::new();
    for i in (0..raw_data.len()).step_by(2) {
        signal_data.push(LittleEndian::read_i16(&raw_data[i..]));
    }

    // TODO CHECK EFFECTS: Float formatting precision - POD5 inspection suggest 13 digits for range (PYTHON) - not sure if necessary
    //
    // We are not using read identifiers, just the channel and numbers - this saves allocation of Strings but means we have to trace outputs by channel/number if 
    // for example cross reference with standard Guppy basecalls - we should not end up doing this because we will evaluate on Dorado calls from BLOW5/POD5 and can
    // modify the identifiers there.
    //
    // This adds a change to the Dorado input stream processing where we did this before by concatenating the channel::number to the read identifier
    // chich is now removed in commit: f08d3aed27818e44d1637f76e8b9cb00c7c79a6b on branch `dori-stdin-v0.3.1` - no changes to output parsing functions necessary
    format!("dori::{}::{} {} {} {} {:.1} {:.11} {} {}\n", channel, number, channel, number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
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

fn request_stream_cache_batch(
    request_stream: Request<tonic::Streaming<DoradoCacheBatchRequest>>,
    mut channel_caches: Vec<HashMap<u32, Vec<Vec<u8>>>>,
    mut pipeline_stdin: ChildStdin,
    init_request: i32,
    cache_request: i32,
    stop_decision: i32,
    config: StreamfishConfig,
    calibration: DeviceCalibration,
    sample_rate: u32
) -> AsyncStream<Result<DoradoCacheResponse, Status>, impl futures::Future<Output = ()>> {

    let mut request_stream = request_stream.into_inner();

    async_stream::try_stream! {


        while let Some(dorado_request) = request_stream.next().await {
            let dorado_request = dorado_request?;
            
            let request_type = dorado_request.request;

            if request_type == init_request {
                // No action, continue and wait for data stream
                continue
            }

            // Per channel processing
            for (channel, read_data) in dorado_request.channels {

                let channel_index = (channel-1) as usize; // need to cast
                let read_cache = &mut channel_caches[channel_index];
                
                // Boolean value is just placeholer for `prost` syntax around `oneof`
                if request_type == cache_request {
                    // If the request is a remove-read-from-cache request, do this
                    // before any further processing - this gets around the mutable
                    // borrow issue below and allows for other decisions to send
                    // cache removal requests later
                    log::info!("Received remove from cache request: {} {}", &channel, &read_data.number);
                    read_cache.remove(&read_data.number);

                    continue;

                } else {

                    // If request is not uncache or initialize, process the input data,
                    // this cloning is annoying but neccessary for the cache operations

                    let number = read_data.number.clone();

                    // Find cached read or insert new one, update the cache with the current read
                    read_cache.entry(read_data.number).or_insert(Vec::new()).push(read_data.raw_data);

                    // Get the updated read and the number of chunks in the cache
                    let cached = read_cache.get(&number).expect("Failed to get data from channel cache");
                    let num_chunks = cached.len();

                    if num_chunks < config.readuntil.read_cache_min_chunks {
                        // If we have less chunks than the minimum number required
                        // send a continue response for more data acquisition on 
                        // this read (no action)
                        continue

                    } else if num_chunks > config.readuntil.read_cache_max_chunks {
                        // If we have reached the maximum chunks in cache,
                        // do not process and send a stop response for
                        // ceasing data acquisition. This will also send
                        // a remove from cache request as we cannot do this
                        // below because ofthe required mutable borrow of the cache

                        // read_cache.remove(&number); not possible!

                        log::info!("Sending stop data decision: {} {} {}", &channel, &number, &num_chunks);

                        let response = DoradoCacheResponse { 
                            channel, 
                            number,
                            decision: stop_decision
                        };
                        yield response

                    } else {
                        // If we have an acceptable number of chunks between the limits, process 
                        // the read by concatenating the chunks and sending it into the basecall
                        // and alignment pipeline 
                        
                        log::info!("Processing cached read: {} {} {}", &channel, &number, &num_chunks);

                        let data: Vec<String> = cached.iter().map(|raw_data| {
                            get_dorado_input_string(
                                raw_data.to_vec(),
                                channel,
                                number,
                                calibration.offsets[channel_index],
                                calibration.pa_ranges[channel_index],
                                calibration.digitisation,
                                sample_rate
                            )
                        }).collect();

                        pipeline_stdin.write_all(data.concat().as_bytes()).await?;

                        // Send a none decision response to take no action after writing into process
                        // this response is mainly for logging 
                        continue
                    }
                }
            }
        }
    }

}