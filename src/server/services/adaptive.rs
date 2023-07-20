
use crate::client::minknow::MinKnowClient;
use crate::config::{StreamfishConfig, MappingExperiment, Experiment, Classifier};
use crate::services::dori_api::adaptive::{StreamfishRequest, StreamfishResponse, Decision, RequestType};
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSampling;

use std::collections::HashMap;
use minimap2::*;
use std::pin::Pin;
use std::sync::Arc;
use itertools::join;
use tokio::sync::Mutex;
use futures_core::Stream;
use futures_util::StreamExt;
use futures_util::AsyncWriteExt;
use futures_util::io::AsyncBufReadExt;
use tonic::{Request, Response, Status};
use byteorder::{LittleEndian, ByteOrder};
use futures_util::io::{BufReader, Lines};
use tokio::sync::mpsc::{Sender, Receiver};
use async_process::{Command, Stdio, ChildStdout, ChildStdin};


pub struct AdaptiveSamplingService {
    config: StreamfishConfig
}
impl AdaptiveSamplingService {
    pub fn new(config: &StreamfishConfig) -> Self {

        if config.readuntil.unblock_all_client {
            log::warn!("Unblocking reads immediately after receipt from control server!");
        }
        if config.readuntil.unblock_all_server {
            log::warn!("Unblocking reads after sending to Dori!");
        }
        if config.readuntil.unblock_all_basecaller {
            log::warn!("Unblocking reads after basecalling on Dori!");
        }
        if config.readuntil.unblock_all_mapper {
            log::warn!("Unblocking reads after basecalling and mapping on Dori!");
        }
        
        log::info!("Basecaller: {} Path: {:?} Args: {:?}", config.dori.basecaller.as_str(), config.dori.basecaller_path, config.dori.basecaller_args.join(" "));
        log::info!("Classifier: {} Path: {:?} Args: {:?}", config.dori.classifier.as_str(), config.dori.classifier_path, config.dori.classifier_args.join(" "));

        Self { config: config.clone() }
    }
}

// Service implementation matches the RPC definitions translated during build - see the trait annotations on BasecallerService
#[tonic::async_trait]
impl AdaptiveSampling for AdaptiveSamplingService {

    type CacheStream = Pin<Box<dyn Stream<Item = Result<StreamfishResponse, Status>> + Send  + 'static>>;
    type StreamStream = Pin<Box<dyn Stream<Item = Result<StreamfishResponse, Status>> + Send  + 'static>>;

    // Adaptive sampling with Dorado basecalling and no request/read cache - single request/read processing only
    async fn stream(&self, _: Request<tonic::Streaming<StreamfishRequest>>) -> Result<Response<Self::StreamStream>, Status> {

        log::info!("Initiated Dori::AdaptiveSamplingService::DoradoStream RPC");

        unimplemented!("No implemented yet")

    }

    // Distinct cache single/batch endpoints because modularizing the stream generators causes tons of latency

    // Adaptive sampling with Dorado basecalling and request/read cache implementation - single channel data transmission
    async fn cache(&self, request: Request<tonic::Streaming<StreamfishRequest>>) -> Result<Response<Self::CacheStream>, Status> {

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
        let init_request: i32 = RequestType::Init.into();

        // Connection to MinKNOW to obtain device calibration data
        let minknow_client = MinKnowClient::connect(
            &self.config.minknow, &self.config.icarust
        ).await.expect("Failed to connect to MinKNOW");

        let (sample_rate, calibration) = minknow_client.get_device_data(
            &self.config.readuntil.device_name, &1, &self.config.readuntil.channels
        ).await.expect("Failed to get device calibration from MinKNOW");

        log::info!("Calibration: {:#?}", calibration);
        
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

        let (cache_queue_tx, mut cache_queue_rx) = tokio::sync::mpsc::channel(1024); // handles cache removal instructions
        let (decision_tx, mut decision_rx) = tokio::sync::mpsc::channel(1024); // drains into output stream
        let (basecall_tx, mut basecall_rx): (Sender<(String, u32, u32, Vec<Vec<u8>>)>, Receiver<(String, u32, u32, Vec<Vec<u8>>)>) = tokio::sync::mpsc::channel(1024); // basecaller stdin

        let uncache_tx = cache_queue_tx.clone();
        let decision_cache_tx = decision_tx.clone();
        
        // Read cache with standard HashMap
        let read_cache: Arc<Mutex<HashMap<String, Vec<Vec<u8>>>>> = Arc::new(Mutex::new(HashMap::new()));
        let read_cache_clone = Arc::clone(&read_cache);
            
        // Uncache thread
        tokio::spawn(async move {
            
            while let Some(read_id) = cache_queue_rx.recv().await {
                let mut read_cache  = read_cache_clone.lock().await;
                
                // This log is useful to ensure cache is cleared continuously -
                // seems to be a decent indicator of things going wrong in general
                log::info!("Reads cached: {} ", read_cache.len());

                read_cache.remove(&read_id);

            }
        
        });

        let channel_start = self.config.readuntil.channel_start.clone();
        let channel_end = self.config.readuntil.channel_end.clone();

        // Basecaller thread
        tokio::spawn(async move {
            
            while let Some((read_id, channel, number, cached)) = basecall_rx.recv().await {
                                
                // There may be a weird race condition with Icarust when 
                // starting all slices in a slice-and-dice configuration
                // simultaneously before Icarust spits out reads after 
                // intials delay (works ok when starting sequentially 
                // when reads have started) causing the wrong slice
                // channel numbers to be transmitted to the slice 
                // causing an out of bounds index error so we 
                // check first here

                // We could also just fet the full channel array 
                // for the calibration (instead of the start - end)
                // so this check won't be necessary, might be better
                
                // if channel > channel_end {
                //     log::warn!("Likely race condition from control server (Icarust) in slice-and-dice!");
                //     log::warn!("Ignore read and continue (channel = {}, channel start = {})", &channel, &channel_start);
                //     continue;
                // }

                let channel_index = (channel - 1) as usize;
                
                log::info!("Channel {} index {} start {}", channel, channel_index, channel_start);

                let data = get_dorado_input_string(
                    read_id,
                    cached.concat(),
                    cached.len(),
                    channel,
                    number,
                    calibration.offsets[channel_index],
                    calibration.pa_ranges[channel_index],
                    calibration.digitisation,
                    sample_rate
                );
                
                pipeline_stdin.write_all(data.as_bytes()).await.expect("Failed to write to pipeline standard input");
            }
        
        });

        // Cache thread
        tokio::spawn(async move {

            while let Some(dorado_request) = request_stream.next().await {
                let dorado_request = dorado_request.unwrap();
                
                if dorado_request.request == init_request {
                    // No action, continue and wait for data stream
                    log::info!("Waiting {} seconds for pipeline initialisation ...", &run_config_1.readuntil.init_delay);
                    tokio::time::sleep(tokio::time::Duration::new(run_config_1.readuntil.init_delay, 0)).await;
                    continue;
                }

                let read_id = dorado_request.id.clone();
                
                let mut cache = read_cache.lock().await;

                // Find cached read or insert new one, update the cache with the current read
                cache.entry(dorado_request.id).or_insert(Vec::new()).push(dorado_request.data);

                // Get the updated read and the number of chunks in the cache
                let cached = cache.get(&read_id).expect("Failed to get data from read cache");
                let num_chunks = cached.len();

                if num_chunks < run_config_1.readuntil.read_cache_min_chunks {
                    continue;
                } else if num_chunks > run_config_1.readuntil.read_cache_max_chunks {

                    decision_cache_tx.send(StreamfishResponse { 
                        channel: dorado_request.channel, 
                        number: dorado_request.number, 
                        decision: stop_decision
                    }).await.expect("Failed to send stop further data into decision response queue");

                    cache_queue_tx.send(read_id).await.expect("Failed to send instruction to uncache thread");

                } else {
                    
                    if run_config_1.readuntil.unblock_all_server {
                        
                        decision_cache_tx.send(StreamfishResponse { 
                            channel: dorado_request.channel, 
                            number: dorado_request.number, 
                            decision: unblock_decision
                        }).await.expect("Failed to send stop further data into decision response queue");

                        cache_queue_tx.send(read_id).await.expect("Failed to send instruction to uncache thread");

                    } else {
                                                
                        basecall_tx.send((read_id, dorado_request.channel, dorado_request.number, cached.to_owned())).await.expect("Failed to send data to basecaller input thread");

                    }
                }
            }
        });

        // =========================
        // Process stream processing
        // =========================
        
        tokio::spawn(async move {

            // Build the mm2 aligner for testing

            let aligner = Aligner::builder()
            .map_ont()
            .with_index(run_config_2.dori.classifier_reference.clone(), None)
            .expect("Unable to build index");
            log::info!("Built the minimap2-rs aligner in stream thread");

            // Initiate variables for line parsing (FASTQ)
            let mut line_counter = 0;

            let mut current_id: String = String::new();
            let mut current_channel: u32 = 0;
            let mut current_number: u32 = 0;
            
            while let Some(line) = pipeline_stdout.next().await {

                let line = line.unwrap();

                // log::info!("{}", line);

                // Dynamic mapping updates require Arc<Mutex<MappingConfig>> locks within the
                // decision/evaluation stream- this does introduce some latency but is
                // not as much as anticipated.

                // let dynamic_mapping_config = dynamic_mapping_config_stream.lock().await; 

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
                        
                        if run_config_2.readuntil.unblock_all_basecaller {
                            
                            log::info!("Unblocking reads from basecaller: {} {}", &current_channel, &current_number);
                            decision_tx.send(StreamfishResponse { 
                                channel: current_channel, 
                                number: current_number, 
                                decision: unblock_decision
                            }).await.expect("Failed to send decision response to queue");
                            uncache_tx.send(current_id.clone()).await.expect("Failed to send uncache identifier to queue");

                            line_counter += 1;
                            continue;
                        }

                        let mappings = aligner.map(line.as_bytes(), false, false, None, None).expect("Failed to map read");
                        
                        // log::info!("{:?}", mappings);

                        if run_config_2.readuntil.unblock_all_mapper {
                            
                            decision_tx.send(StreamfishResponse { 
                                channel: current_channel, 
                                number: current_number, 
                                decision: unblock_decision
                            }).await.expect("Failed to send decision response to queue");

                            uncache_tx.send(current_id.clone()).await.expect("Failed to send uncache identifier to queue");

                            line_counter += 1;
                            continue;
                        }

                        // log::info!("{:?}", line);

                        // DO NOT LOG THE MAPPINGS OR USE BORROWS ON THEM - FOR SOME ULTRA ARCANE REASON THIS BRAKES EVERYTHING

                        // MAYBE THIS IS MUSL BUILD RELATED? I THINK IT IS - NORMAL X86_64 BUILDS DO NOT RUN WILD EXCEPT FOR MISCONFIGURED EXPERIMENTS
                        
                        let decision = mapping_config.decision_from_mapping(mappings);

                        if decision == stop_decision || decision == unblock_decision {
                            uncache_tx.send(current_id.clone()).await.expect("Failed to send uncache identifier to queue");
                        }
                        
                        decision_tx.send(StreamfishResponse { 
                            channel: current_channel, 
                            number: current_number, 
                            decision: decision
                        }).await.expect("Failed to send decision response to queue");

                        line_counter += 1;

                    } else {
                        line_counter += 1;
                    }
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

        Ok(Response::new(Box::pin(pipeline_response_stream) as Self::CacheStream))

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




pub fn get_dorado_input_string(id: String, raw_data: Vec<u8>, chunks: usize, channel: u32, number: u32, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

    // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
    let mut signal_data: Vec<i16> = Vec::new();
    for i in (0..raw_data.len()).step_by(2) {
        signal_data.push(LittleEndian::read_i16(&raw_data[i..]));
    }

    format!("{}::{}::{}::{} {} {} {} {:.1} {:.11} {} {}\n", id, channel, number, chunks, channel, number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
}
