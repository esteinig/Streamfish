
use crate::client::minknow::MinKnowClient;
use crate::config::{StreamfishConfig, MappingExperiment, Experiment, Classifier};
use crate::services::dori_api::adaptive::{DoradoStreamRequest, DoradoStreamResponse};
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSampling;
use crate::services::dori_api::adaptive::{
    DoradoCacheBatchRequest, 
    DoradoCacheChannelRequest, 
    DoradoCacheResponse, 
    Decision, 
    DoradoCacheRequestType
};

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

use std::sync::Arc;

use moka::future::Cache;

use minimap2::*;

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

        if config.readuntil.unblock_all_client {
            log::warn!("Unblocking reads immediately after receipt from control server!");
        }
        if config.readuntil.unblock_all_server {
            log::warn!("Unblocking reads after sending to Dori!");
        }
        if config.readuntil.unblock_all_process {
            log::warn!("Unblocking reads after processing on Dori!");
        }
        
        log::info!("Basecaller: {} Path: {:?} Args: {:?}", config.dori.basecaller.as_str(), config.dori.basecaller_path, config.dori.basecaller_args.join(" "));
        log::info!("Classifier: {} Path: {:?} Args: {:?}", config.dori.classifier.as_str(), config.dori.classifier_path, config.dori.classifier_args.join(" "));

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

    // Distinct cache single/batch endpoints because modularizing the stream generators causes tons of latency

    // Adaptive sampling with Dorado basecalling and request/read cache implementation - single channel data transmission
    async fn dorado_cache_channel(&self, request: Request<tonic::Streaming<DoradoCacheChannelRequest>>) -> Result<Response<Self::DoradoCacheChannelStream>, Status> {

        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoCacheChannel RPC");

        // Used in generators, needs distinct clones
        let run_config_1 = self.config.clone();
        let run_config_2 = self.config.clone();

        // Adaptive sampling experiment configuration
        let experiment = self.config.experiment.config.clone();

        let mapping_config = match experiment {
            Experiment::MappingExperiment(MappingExperiment::HostDepletion(host_depletion_config)) => host_depletion_config,
            Experiment::MappingExperiment(MappingExperiment::TargetedSequencing(targeted_sequencing_config)) => targeted_sequencing_config,
            Experiment::MappingExperiment(MappingExperiment::UnknownSequences(unknown_config)) => unknown_config
        };

        log::info!("Mapping configuration: {:#?}", mapping_config);

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Action sent to MinKNOW
        let stop_decision: i32 = Decision::StopData.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // Request types
        let init_request: i32 = DoradoCacheRequestType::Init.into();

        // Connection to MinKNOW to obtain device calibration data
        let minknow_client = MinKnowClient::connect(
            &self.config.minknow, &self.config.icarust
        ).await.expect("Failed to connect to MinKNOW");

        let (sample_rate, calibration) = minknow_client.get_device_data(
            &self.config.readuntil.device_name, 
            &self.config.readuntil.channel_start,  
            &self.config.readuntil.channel_end
        ).await.expect("Failed to get device calibration from MinKNOW");
        


        // ======================
        // Pipeline process setup
        // ======================

        let (mut pipeline_stdin, mut pipeline_stdout) = init_pipeline(&self.config);

        // ======================================
        // Dynamic loop for mapping config update
        // ======================================

        // let dynamic_mapping_config_main = std::sync::Arc::new(tokio::sync::Mutex::new(mapping_config));
        // let dynamic_mapping_config_thread = std::sync::Arc::clone(&dynamic_mapping_config_main);
        // let dynamic_mapping_config_stream = std::sync::Arc::clone(&dynamic_mapping_config_main);
        
        // tokio::spawn(async move {
        //     loop {
        //         tokio::time::sleep(tokio::time::Duration::new(5, 0)).await;

        //         let mut dynamic_mapping_config_lock = dynamic_mapping_config_thread.lock().await;
                
        //         // Switches between chr1 and chr3 and chr9 and chr11 every few seconds for testing
        //         if dynamic_mapping_config_lock.targets.contains(&String::from("chr1")) {
        //             dynamic_mapping_config_lock.targets = Vec::from([String::from("chr9"), String::from("chr11")]);
        //         } else {
        //             dynamic_mapping_config_lock.targets = Vec::from([String::from("chr1"), String::from("chr3")]);
        //         }
        //     }           
        // });

        // =========================
        // Request stream processing
        // =========================
     
        // DO NOT PUT STREAM GENERATOR INTO DISTINCT FUNCTION - SEVERELY IMPACTS LATENCY (> 100 bp)

        
        let mut request_stream = request.into_inner();

        // ADDING A QUEUE TX/RX INTO THE ASYNC STREAM GENERATORS AND THREADING THE REQUEST PROCESS LOOPS ADDS LATENCY (~ 7 bp)
        // 
        // Threading the writes to the basecaller does not make a difference despite all the ArcMutex and cloning going on
        // keeping it simple instead and removing the threaded request parsing in the stream generator. NOT using Dorado
        // alignment keeps things stable at the aligner end - even when increasing to 1024 pores and 256 batch size
        // which adds a bit of latency due to larger batch size (~40 bp) - however when using alignment in Dorado it 
        // becomes unstable with or without writer async threads, and batch size needs to be > 512 adding significant
        // latency overall.

        let (cache_queue_tx, mut cache_queue_rx) = tokio::sync::mpsc::channel(1024); // handles cache removal instructions
        let (decision_tx, mut decision_rx) = tokio::sync::mpsc::channel(1024); // drains into output stream

        let uncache_tx = cache_queue_tx.clone();
        let cache_decision_tx = decision_tx.clone();
    
        // Use channel specific caches due to not having to use a string value for lookup
        // let read_cache: Cache<String, Arc<Vec<Vec<u8>>>> = Cache::new(10_000);
        
        // A self clearing cache - this is important - the simple HashMap version previously
        // implemented was not able to be cleared, because the incomping reads updated the
        // cache too quickly OR because the ArcMutex lock was hogged by the input stream 
        // - both causing instabilites. With the Arc values implemented below we only 
        // clone the Arc on gets and set expiration times by which we expect
        // chunks to be outdated - TTL is since insert, TTI is since last get
        let read_cache: Cache<String, Arc<Vec<Vec<u8>>>> = Cache::builder()
                // Time to live (TTL): 
                .time_to_live(std::time::Duration::from_secs(3))
                // Time to idle (TTI):
                .time_to_idle(std::time::Duration::from_secs(1))
                // Create the cache.
                .build();

        let read_cache_1 = read_cache.clone();
        let read_cache_2 = read_cache.clone();

        // Cache removal thread - receives messages from pipeline processing stream
        tokio::spawn(async move {
            
            while let Some(read_id) = cache_queue_rx.recv().await {

                read_cache_2.remove(&read_id).await;
                log::info!("Reads cached: {} ", read_cache_2.entry_count());
            }
        
        });

        // READ CACHE IS THE PROBLEM - FILLS UP FASTER THAN IT IS EMPTIED! PROBABLY LOCKED MORE BELOW THAN ABOVE

        tokio::spawn(async move {


            while let Some(dorado_request) = request_stream.next().await {
                let dorado_request = dorado_request.unwrap();
                
                let read_id = dorado_request.id.clone();

                if dorado_request.request == init_request {
                    // No action, continue and wait for data stream
                    log::info!("Waiting {} seconds for pipeline initialisation ...", &run_config_1.readuntil.init_delay);
                    tokio::time::sleep(tokio::time::Duration::new(run_config_1.readuntil.init_delay, 0)).await;
                    continue;
                }

                // Looks weird, but actually works this way
                //
                // https://github.com/moka-rs/moka#example-size-aware-eviction
                let cached = match read_cache_1.get(&read_id) {
                    Some(data) => {
                        let cached = &*data;
                        let mut updated = Vec::new();
                        for chunk in cached {
                            updated.push(chunk.to_owned())
                        }
                        updated.push(dorado_request.data);
                        read_cache.insert(dorado_request.id, Arc::new(updated.clone())).await;
                        updated
                    },
                    None => {
                        let data = vec![dorado_request.data];
                        read_cache.insert(dorado_request.id, Arc::new(data.clone())).await;
                        data
                    }
                };
                
                let num_chunks = cached.len();

                if num_chunks < run_config_1.readuntil.read_cache_min_chunks {
                    // If we have less chunks than the minimum number required
                    // no action is taken, simply continue parsing requests until
                    // a sufficient number of chunks is available
                    continue;

                } else if num_chunks > run_config_1.readuntil.read_cache_max_chunks {
                    // If we have reached the maximum chunks in cache,
                    // do not process and send a stop response for
                    // ceasing data acquisition. This will also send
                    // a remove from cache request as we cannot do this
                    // below because of the required mutable borrow of the cache

                    // read_cache.remove(&number); not possible!

                    log::debug!("Maximum chunk size exceeded, stop data decision for: {} (chunks = {})", &read_id, &num_chunks);
                    cache_decision_tx.send(DoradoCacheResponse { 
                        channel: dorado_request.channel, 
                        number: dorado_request.number, 
                        decision: stop_decision
                    }).await.expect("Failed to send stop further data into decision response queue");

                    log::debug!("Sending remove-from-cache instructions: {} (chunks = {})", &read_id, &num_chunks);
                    cache_queue_tx.send(read_id).await.expect("Failed to send uncache instruction to queue");

                } else {
                    // If we have an acceptable number of chunks between the limits, process 
                    // the read by concatenating the chunks and sending it into the basecall
                    // and alignment pipeline 
                    
                    // If unblock-all testing is active, send response after channel cache decisions
                    if run_config_1.readuntil.unblock_all_server {
                        
                        log::debug!("Unblock all active before processing, unblock decision before: {} (chunks = {})", &read_id, &num_chunks);
                        cache_decision_tx.send(DoradoCacheResponse { 
                            channel: dorado_request.channel, 
                            number: dorado_request.number, 
                            decision: unblock_decision
                        }).await.expect("Failed to send stop further data into decision response queue");
                    } else {
                                                    
                        log::debug!("Process cached read data and send into pipeline: {} (chunks = {})", &read_id, &num_chunks);
                        
                        let channel_index = (dorado_request.channel - 1) as usize;

                        let data: Vec<String> = cached.into_iter().map(|raw_data| {
                            get_dorado_input_string(
                                read_id.clone(),
                                raw_data,
                                dorado_request.channel,
                                dorado_request.number,
                                calibration.offsets[channel_index],
                                calibration.pa_ranges[channel_index],
                                calibration.digitisation,
                                sample_rate
                            )
                        }).collect();
                        

                        pipeline_stdin.write_all(data.concat().as_bytes()).await.expect("Failed to write to pipeline standard input");
                    }
                }
            }
        });

            // while let Some(dorado_request) = request_stream.next().await {
            //     let dorado_request = dorado_request.unwrap();
                
            //     let channel = dorado_request.channel;
            //     let number = dorado_request.number;
    
            //     if dorado_request.request == init_request {
            //         // No action, continue and wait for data stream
            //         log::info!("Waiting {} seconds for pipeline initialisation ...", &run_config_1.readuntil.init_delay);
            //         tokio::time::sleep(tokio::time::Duration::new(run_config_1.readuntil.init_delay, 0)).await;

            //         // No response is sent
            //         continue;
            //     }
                
            //     let channel_index = (channel-1) as usize; // need to cast
            //     let read_cache = &mut channel_caches[channel_index];
                
            //     // Boolean value is just placeholer for `prost` syntax around `oneof`
            //     if dorado_request.request == cache_request {
            //         // If the request is a remove-read-from-cache request, do this
            //         // before any further processing - this gets around the mutable
            //         // borrow issue below and allows for other decisions to send
            //         // cache removal requests later
            //         log::debug!("Removing chunks from cache on request: {} {}", &channel, &number);
            //         read_cache.remove(&number);

            //         // No response is sent
            //         continue;

            //     } else {

            //         // Find cached read or insert new one, update the cache with the current read
            //         read_cache.entry(number).or_insert(Vec::new()).push(dorado_request.data);

            //         // Get the updated read and the number of chunks in the cache
            //         let cached = read_cache.get(&number).expect("Failed to get data from channel cache");
            //         let num_chunks = cached.len();

            //         if num_chunks < run_config_1.readuntil.read_cache_min_chunks {
            //             // If we have less chunks than the minimum number required
            //             // send a continue response for more data acquisition on 
            //             // this read (no action)

            //             // No response is sent
            //             continue;

            //         } else if num_chunks > run_config_1.readuntil.read_cache_max_chunks {
            //             // If we have reached the maximum chunks in cache,
            //             // do not process and send a stop response for
            //             // ceasing data acquisition. This will also send
            //             // a remove from cache request as we cannot do this
            //             // below because of the required mutable borrow of the cache

            //             // read_cache.remove(&number); not possible!

            //             log::debug!("Maximum chunk size exceeded, stop data decision: {} {} (chunks = {})", &channel, &number, &num_chunks);

            //             yield DoradoCacheResponse { 
            //                 channel, 
            //                 number,
            //                 decision: stop_decision
            //             }

            //         } else {
            //             // If we have an acceptable number of chunks between the limits, process 
            //             // the read by concatenating the chunks and sending it into the basecall
            //             // and alignment pipeline 
                        
            //             // If unblock-all testing is active, send response after channel cache decisions
            //             if run_config_1.readuntil.unblock_all_server {
            //                 yield DoradoCacheResponse { 
            //                     channel, 
            //                     number,
            //                     decision: unblock_decision
            //                 }

            //             } else {
                                                        
            //                 log::debug!("Processing cached read: {} {} (chunks = {})", &channel, &number, &num_chunks);

            //                 let data: Vec<String> = cached.iter().map(|raw_data| {
            //                     get_dorado_input_string(
            //                         raw_data.to_vec(),
            //                         channel,
            //                         number,
            //                         calibration.offsets[channel_index],
            //                         calibration.pa_ranges[channel_index],
            //                         calibration.digitisation,
            //                         sample_rate
            //                     )
            //                 }).collect();
                            

            //                 pipeline_stdin.write_all(data.concat().as_bytes()).await.expect("Failed to write to pipeline standard input");
                            
            //                 // No response is sent
            //                 continue;
            //             }
            //         }
            //     }
        


        // =========================
        // Process stream processing
        // =========================

        // Async queue to yield decision responses into the stream
        
        // SAM or FASTQ output from Dorado
        let sam_output = match self.config.dori.classifier { 
            Classifier::Minimap2Dorado => true,
             _ => false 
        };


        tokio::spawn(async move {

            // Build the mm2 aligner for testing

            let aligner = Aligner::builder()
            .map_ont()
            .with_index(run_config_2.dori.classifier_reference_path.clone(), None)
            .expect("Unable to build index");
            log::info!("Built the minimap2-rs aligner in stream thread");

            // Initiate variables for line parsing (FASTQ)
            let mut line_counter = 0;

            let mut current_id: String = String::new();
            let mut current_channel: u32 = 0;
            let mut current_number: u32 = 0;
            
            while let Some(line) = pipeline_stdout.next().await {

                let line = line.unwrap();

                // Dynamic mapping updates require Arc<Mutex<MappingConfig>> locks within the
                // decision/evaluation stream- this does introduce some latency but is
                // not as much as anticipated.

                // let dynamic_mapping_config = dynamic_mapping_config_stream.lock().await; 

                match sam_output {
                    // FASTQ OUTPUT FROM DORADO - UNBLOCKS ALL
                    false => {

                        if line_counter == 0 {

                            // Parse
                            let identifiers: Vec<&str> = line.split("::").collect();

                            current_id = identifiers[0].trim_start_matches("@").to_string();
                            current_channel = identifiers[1].parse::<u32>().unwrap();
                            current_number = identifiers[2].parse::<u32>().unwrap();

                            line_counter += 1;

                        } else {
                            if line_counter == 3 {
                                // Reset line counter of FASTQ
                                line_counter = 0;
                            } else if line_counter == 1 {
                                
                                // Get the sequence and align it in async thread

                                // let response_tx = decision_tx.clone();
                                // let aligner_clone = aligner.clone();
                                // let c = current_channel.clone();
                                // let n = current_number.clone();

                                let mappings = aligner.map(line.as_bytes(), false, false, None, None).expect("Failed to map read");
                                
                                // log::info!("{:?}", mappings);

                                // DO NOT LOG THE MAPPINGS OR USE BORROWS ON THEM - FOR SOME ULTRA ARCANE REASON THIS BRAKES EVERYTHING

                                // MAYBE THIS IS MUSL BUILD RELATED? I THINK IT IS - NORMAL X86_64 BUILDS DO NOT RUN WILD EXCEPT FOR MISCONFIGURED EXPERIMENTS

                                let decision = mapping_config.decision_from_mapping(mappings, run_config_2.readuntil.unblock_all_process);

                                if decision == stop_decision || decision == unblock_decision {
                                    uncache_tx.send(current_id.clone()).await.expect("Failed to send uncache identifier to queue");
                                }
                               
                                decision_tx.send(DoradoCacheResponse { 
                                    channel: current_channel, 
                                    number: current_number, 
                                    decision: decision
                                }).await.expect("Failed to send decision response to queue");

                                // tokio::spawn(async move {
                                //     let mapping = aligner_clone.map(line.as_bytes(), false, false, None, None).expect("Failed to map read");

                                //     let decision = match run_config_2.readuntil.unblock_all_process {
                                //         true => map_conf.unblock_all(&0, ""),
                                //         false => map_conf.decision_from_mapping(&mapping)
                                //     };

                                //     response_tx.send(DoradoCacheResponse { 
                                //         channel: c, 
                                //         number: n, 
                                //         decision: decision
                                //     }).await.expect("Failed to send decision response to queue");
                                
                                // });
                                

                                line_counter += 1;

                            } else {
                                line_counter += 1;
                            }
                        }
                    },
                    true => {
                        // SAM OUTPUTS USING MINIMAP2-DORADO - REDUCED LATENCY WITH SMALL REFERENCE INDEX
                        // 
                        // !WARNING! - WHEN STREAMING STDIN/STDOUT DORADO ALIGNMENT IS UNSTABLE WITH LARGER REFERENCE INDICES - !WARNING!
                        //
                        // Weirdly, instability also occurs when providing no reference but emitting SAM (--emit-sam) - something may
                        // be interacting with my hacky stream implementation of Dorado.
                        //
                        // Actually now that the decision queue and threaded parser are implemented it appears that the responses with
                        // Dorado SAM are also stable with larger reference indices - maybe in these async streams, we need to always
                        // use the queues and thread the output parser?

                        if line.starts_with('@') { 
                            continue; 
                        }
                        let content: Vec<&str> = line.split('\t').collect();
                        let identifiers: Vec<&str> = content[0].split("::").collect();

                        let flag = content[1].parse::<u32>().unwrap();
                        let tid = content[2];
                        
                        let decision = mapping_config.decision_from_sam(&flag, tid);

                        log::info!("mapped={:<21} flag={:<4} action={:<1} ", tid, &flag, &decision);

                        decision_tx.send(DoradoCacheResponse { 
                            channel: identifiers[1].parse::<u32>().unwrap(), 
                            number: identifiers[2].parse::<u32>().unwrap(), 
                            decision
                        }).await.expect("Failed to send decision response to queue");
                    },
                    
                }
            }
        });

        let pipeline_response_stream = async_stream::try_stream! {
            while let Some(response) = decision_rx.recv().await {
                yield response
            }
        };


        // =========================
        // Response stream merge
        // =========================

        // let response_stream = futures::stream::select(
        //     request_response_stream,
        //     pipeline_response_stream, 
        // );

        Ok(Response::new(Box::pin(pipeline_response_stream) as Self::DoradoCacheChannelStream))

    }

    // Adaptive sampling with Dorado basecalling and request/read cache implementation - batched channel data transmission
    async fn dorado_cache_batch(&self, request: Request<tonic::Streaming<DoradoCacheBatchRequest>>) -> Result<Response<Self::DoradoCacheBatchStream>, Status> {
        
        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoCacheBatch RPC");

        // Used in generators, needs distinct clones
        let run_config_1 = self.config.clone();
        let run_config_2 = self.config.clone();

        // Experiment config - see how to update this one
        let experiment = self.config.experiment.config.clone();

        let mapping_config = match experiment {
            Experiment::MappingExperiment(MappingExperiment::HostDepletion(host_depletion_config)) => host_depletion_config,
            Experiment::MappingExperiment(MappingExperiment::TargetedSequencing(targeted_sequencing_config)) => targeted_sequencing_config,
            Experiment::MappingExperiment(MappingExperiment::UnknownSequences(unknown_config)) => unknown_config
        };

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Action sent to MinKNOW
        let stop_decision: i32 = Decision::StopData.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // Request types
        let init_request: i32 = DoradoCacheRequestType::Init.into();
        let cache_request: i32 = DoradoCacheRequestType::Cache.into();

        // Connection to MinKNOW to obtain device calibration data
        let minknow_client = MinKnowClient::connect(
            &self.config.minknow, &self.config.icarust
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

        let (mut pipeline_stdin, mut pipeline_stdout) = init_pipeline(&self.config);

        // =========================
        // Request stream processing
        // =========================
     
        // DO NOT PUT STREAM GENERATOR INTO DISTINCT FUNCTION - SEVERELY IMPACTS LATENCY (> 100 bp)

        let mut request_stream = request.into_inner();
        
        let request_response_stream = async_stream::try_stream! {
    
            while let Some(dorado_request) = request_stream.next().await {
                let dorado_request = dorado_request?;
                
                let request_type = dorado_request.request;
    
                if request_type == init_request {
                    // No action, continue and wait for data stream
                    log::info!("Waiting {} seconds for pipeline initialisation ...", &run_config_1.readuntil.init_delay);
                    tokio::time::sleep(tokio::time::Duration::new(run_config_1.readuntil.init_delay, 0)).await;
                    continue;
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
                        log::debug!("Received remove from cache request: {} {}", &channel, &read_data.number);
                        read_cache.remove(&read_data.number);
    
                        continue;
    
                    } else {
    
                        let number = read_data.number;
    
                        // Find cached read or insert new one, update the cache with the current read
                        read_cache.entry(read_data.number).or_insert(Vec::new()).push(read_data.raw_data);
    
                        // Get the updated read and the number of chunks in the cache
                        let cached = read_cache.get(&number).expect("Failed to get data from channel cache");
                        let num_chunks = cached.len();
    
                        if num_chunks < run_config_1.readuntil.read_cache_min_chunks {
                            // If we have less chunks than the minimum number required
                            // send a continue response for more data acquisition on 
                            // this read (no action)
                            continue;
    
                        } else if num_chunks > run_config_1.readuntil.read_cache_max_chunks {
                            // If we have reached the maximum chunks in cache,
                            // do not process and send a stop response for
                            // ceasing data acquisition. This will also send
                            // a remove from cache request as we cannot do this
                            // below because of the required mutable borrow of the cache
    
                            // read_cache.remove(&number); not possible!
                            log::info!("Sending stop data decision: {} {} (chunks = {})", &channel, &number, &num_chunks);

                            yield  DoradoCacheResponse { 
                                channel, 
                                number,
                                decision: stop_decision
                            }
    
                        } else {
                            // If we have an acceptable number of chunks between the limits, process 
                            // the read by concatenating the chunks and sending it into the basecall
                            // and alignment pipeline 


                            // If unblock-all testing is active, send
                            // response after channel cache decisions
                            if run_config_1.readuntil.unblock_all_server {
                                yield DoradoCacheResponse { 
                                    channel, 
                                    number,
                                    decision: unblock_decision
                                }
                            } else {
                                log::debug!("Processing cached read: {} {} (chunks = {})", &channel, &number, &num_chunks);
    
                                let data: Vec<String> = cached.iter().map(|raw_data| {
                                    get_dorado_input_string(
                                        "dori".to_string(),
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
                                continue;
                            }
                            
                        }
                    }
                }
            }
        };
        

        // =========================
        // Process stream processing
        // =========================

        let sam_output = true;
        let pipeline_response_stream = async_stream::try_stream! {

            while let Some(line) = pipeline_stdout.next().await {
                let line = line?;

                match sam_output {
                    // SAM HEADER OUTPUTS - MAPPING CONFIGURATION
                    true => {
                        if line.starts_with('@') { 
                            continue; 
                        }
                        let content: Vec<&str> = line.split('\t').collect();
                        let identifiers: Vec<&str> = content[0].split("::").collect();

                        let flag = content[1].parse::<u32>().unwrap();
                        let tid = content[2];

                        let decision = match run_config_2.readuntil.unblock_all_process {
                            true => mapping_config.unblock_all(&flag, tid),
                            false => mapping_config.decision_from_sam(&flag, tid)
                        };

                        log::info!("mapped={:<21} flag={:<4} action={:<1} targets={:?}", tid, &flag, &decision, mapping_config.targets);

                        yield DoradoCacheResponse { 
                            channel: identifiers[1].parse::<u32>().unwrap(), 
                            number: identifiers[2].parse::<u32>().unwrap(), 
                            decision
                        }
                    },
                    // FASTQ THROUGHPUT FOR TESTING - UNBLOCKS ALL
                    false =>  {
                        continue                        
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

    log::info!("Spawning processes in analysis pipeline...");

    let process_stderr = Stdio::from(std::fs::File::create(config.dori.stderr_log.clone()).expect(
        &format!("Failed to create basecaller stderr file: {}", config.dori.stderr_log.display())
    ));

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




use byteorder::{LittleEndian, ByteOrder};

pub fn get_dorado_input_string(id: String, raw_data: Vec<u8>, channel: u32, number: u32, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

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
    format!("{}::{}::{} {} {} {} {:.1} {:.11} {} {}\n", id, channel, number, channel, number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
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
