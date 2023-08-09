
use minimap2::*;
use std::pin::Pin;
use itertools::join;
use moka::future::Cache;
use futures_core::Stream;
use futures_util::StreamExt;
use futures_util::AsyncWriteExt;
use futures_util::io::AsyncBufReadExt;
use tonic::{Request, Response, Status};
use byteorder::{LittleEndian, ByteOrder};
use futures_util::io::{BufReader, Lines};
use tokio::sync::mpsc::{Sender, Receiver};
use crate::client::dori::DynamicFeedbackClient;
use crate::client::minknow::MinknowClient;
use crate::config::Target;
use crate::config::{StreamfishConfig, Basecaller};
use crate::services::dori_api::dynamic::{DynamicFeedbackRequest, RequestType as DynamicRequestType};
use async_process::{Command, Stdio, ChildStdout, ChildStdin};
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSampling;
use crate::services::dori_api::adaptive::{StreamfishRequest, StreamfishResponse, Decision, RequestType as AdaptiveRequestType};
use crate::services::dori_api::dynamic::DynamicTarget;


pub struct FastxRecord {
    pub channel: u32,
    pub number: u32,
    pub id: String,
    pub seq: String
}

// Initiates the process pipeline from configuration
fn init_pipeline(config: &StreamfishConfig) -> Result<(ChildStdin, Lines<BufReader<ChildStdout>>), Status> {

    log::info!("Spawning pipeline process...");

    let stderr_file = match std::fs::File::create(config.dori.adaptive.stderr_log.clone()) {
        Ok(file) => file,
        Err(_) => return Err(Status::internal("Failed to create pipeline stderr file"))
    };

    let process_stderr = Stdio::from(stderr_file);

    match config.dori.adaptive.basecaller {
        Basecaller::Guppy => {
            let mut pipeline_process = match Command::new(config.guppy.client.path.as_os_str())
                .args(config.guppy.client.args.clone())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(process_stderr)
                .spawn() 
            {
                    Ok(process) => process,
                    Err(_) => return Err(Status::internal("Failed to spawn pipeline process"))
            };

            let stdin = match pipeline_process.stdin.take() {
                Some(stdin) => stdin,
                None => return Err(Status::internal("Failed to open pipeline process stdin"))
            };
            let stdout = match pipeline_process.stdout.take() {
                Some(stdout) => stdout,
                None => return Err(Status::internal("Failed to open pipeline process stdout"))
            };
            
            Ok((stdin, BufReader::new(stdout).lines()))

        },
        _ => unimplemented!("Dorado is not implemented yet!")
    }
    
}

// Get the Dorado input string for the modified STDIN Dorado v0.3.1 fork
pub fn get_dorado_input_string(id: String, raw_data: Vec<u8>, chunks: usize, channel: u32, number: u32, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

    // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
    let mut signal_data: Vec<i16> = Vec::new();
    for i in (0..raw_data.len()).step_by(2) {
        signal_data.push(LittleEndian::read_i16(&raw_data[i..]));
    }

    format!("{}::{}::{}::{} {} {} {} {:.1} {:.11} {} {}\n", id, channel, number, chunks, channel, number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
}


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
        
        log::debug!("Basecall client: {} Path: {:?} Args: {:?}", config.dori.adaptive.basecaller.as_str(), config.guppy.client.path.display(), config.guppy.client.args.join(" "));
        log::debug!("Basecall server: {} Path: {:?} Args: {:?}", config.dori.adaptive.basecaller.as_str(), config.guppy.server.path.display(), config.guppy.server.args.join(" "));

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

        log::info!("Dori::AdaptiveSamplingService::Cache RPC");

        // Used in generators, needs distinct clones
        let run_config_1 = self.config.clone();
        let run_config_2 = self.config.clone();

        // Adaptive sampling experiment configuration
        let experiment = self.config.experiment.experiment.clone();

        let mapping_config = experiment.get_mapping_config();

        log::info!("Loaded mapping configuration");

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Action sent to MinKNOW
        let stop_decision: i32 = Decision::StopData.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // Request types
        let init_request: i32 = AdaptiveRequestType::Init.into();

        // Connection to MinKNOW to obtain device calibration data
        let minknow_client = match MinknowClient::connect(
            &self.config.minknow, Some(self.config.icarust.clone())
        ).await {
            Ok(client) => client,
            Err(_) => return Err(Status::internal("Failed to connect to control server"))
        };


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

        let (sample_rate, calibration) = match minknow_client.get_device_data(
            &self.config.readuntil.device_name, &1, &self.config.readuntil.channels
        ).await {
            Ok(data) => data,
            Err(_) => return Err(Status::internal("Failed to get calibration data from control server"))
        };

        // ======================
        // Pipeline process setup
        // ======================

        let (mut pipeline_stdin, mut pipeline_stdout) = init_pipeline(&self.config)?;

        // ======================================
        // Dynamic loop for mapping config update
        // ======================================


        let dynamic_mapping_config_main = std::sync::Arc::new(tokio::sync::Mutex::new(mapping_config.clone()));
        let dynamic_mapping_request_stream_thread = std::sync::Arc::clone(&dynamic_mapping_config_main);
        let dynamic_mapping_response_stream_thread = std::sync::Arc::clone(&dynamic_mapping_config_main);

        // Main thread for decisions below
        let dynamic_mapping_config_stream = std::sync::Arc::clone(&dynamic_mapping_config_main);
        
        // Dynamic configuration cache that stores read identifiers from decisions
        let dynamic_cache: Cache<String, String> = Cache::builder()
            .max_capacity(self.config.dynamic.cache_capacity)
            .build();

        let dynamic_task_cache_request_stream = dynamic_cache.clone();
        let dynamic_task_cache_decision = dynamic_cache.clone();
        let dynamic_task_cache_chunks = dynamic_cache.clone();


        // ===================================
        // Dynamic client stream to server RPC
        // ===================================

        if self.config.dynamic.enabled {

            let config = self.config.clone();

            log::info!("Connecting to dynamic feedback service...");
            let mut dori_rpc = match DynamicFeedbackClient::connect(&config).await {
                Ok(client) => client,
                Err(_) => return Err(Status::internal("Failed to connect to dynamic feedback service on Dori"))
            };
            log::info!("Connected to dynamic feedback service");

            let (request_queue_tx, mut request_queue_rx) = tokio::sync::mpsc::channel(1024); // handles request stream to dynamic feedback server

            // Setup the action request stream from this client - this stream reveives 
            // messages from the action queue (`GetLiveReadsRequests`)
            let dynamic_request_stream = async_stream::stream! {
                while let Some(request) = request_queue_rx.recv().await {
                    yield request;
                }
            };
            log::info!("Sending initiation request to dynamic stream");
            // Initiation request to stream, otherwise blocks flow:
            request_queue_tx.send(DynamicFeedbackRequest { request: DynamicRequestType::Init.into(), targets: Vec::new(), reads: Vec::new() }).await.expect("Failed to send request into dynamic feedback stream");

            
            log::info!("Establishing dynamic feedback stream");
            let mut response_stream = dori_rpc.client.test(Request::new(dynamic_request_stream)).await?.into_inner();
            log::info!("Dynamic feedback stream established");
            

            // Stream input task
            tokio::spawn(async move {
                
                // Interval loop - only send a request every so often...
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(config.dynamic.interval_seconds)).await;
                    
                    // Get all read identifiers from cache and clear it
                    //
                    // We are not performance bound in this thread since this is the "slow real-time" analysis loop
                    // for the iteration see the Guarantees section for Moka: 
                    //
                    // https://docs.rs/moka/latest/moka/future/struct.Cache.html#method.iter

                    let read_ids: Vec<String> = dynamic_task_cache_request_stream.iter().map(|(_, v)| v).collect();

                    // Clear the cache in a background task - we should check if reads are lost [TODO]
                    dynamic_task_cache_request_stream.invalidate_all();

                    // Unlock the mapping config and get current dynamic targets for the request
                    let dynamic_mapping_config = dynamic_mapping_request_stream_thread.lock().await;
                    let dynamic_targets: Vec<DynamicTarget> = dynamic_mapping_config.targets.iter().map(|t| t.to_dynamic_target()).collect();

                    request_queue_tx.send(DynamicFeedbackRequest { request: DynamicRequestType::Data.into(), targets: dynamic_targets, reads: read_ids }).await.expect("Failed to send request into dynamic feedback stream");
                }

                Ok::<(), Status>(())  // we need this type annotation but have an infinite loop
            });


            // Stream ouput task
            tokio::spawn(async move {
                
                while let Some(response) = response_stream.message().await? {

                    let mut dynamic_mapping_config = dynamic_mapping_response_stream_thread.lock().await;
                    let dynamic_targets = response.targets.iter().map(Target::from_dynamic_target).collect();

                    // Create the config targets from the updated dynamic target response and update mapping config
                    dynamic_mapping_config.targets = dynamic_targets;

                    log::warn!("Switched to targets: {:?}", dynamic_mapping_config.targets);

                }
                

            Ok::<(), Status>(()) 
        });


        }

        

        // =========================
        // Request stream processing
        // =========================
     
        // DO NOT PUT STREAM GENERATOR INTO DISTINCT FUNCTION - SEVERELY IMPACTS LATENCY (> 100 bp)
        
        let mut request_stream = request.into_inner();

        let (cache_queue_tx, mut cache_queue_rx) = tokio::sync::mpsc::channel(1024); // handles cache removal instructions
        let (decision_tx, mut decision_rx) = tokio::sync::mpsc::channel(1024); // drains into output stream
        let (basecall_tx, mut basecall_rx): (Sender<(String, u32, u32, Vec<Vec<u8>>)>, Receiver<(String, u32, u32, Vec<Vec<u8>>)>) = tokio::sync::mpsc::channel(1024); // basecaller stdin submission
        let (alignment_tx, mut alignment_rx) = tokio::sync::mpsc::channel(1024); // basecaller fastx processing


        let uncache_basecaller_tx = cache_queue_tx.clone();
        let uncache_aligner_tx = cache_queue_tx.clone();
        let uncache_cache_tx = cache_queue_tx.clone();

        let decision_basecaller_tx = decision_tx.clone();
        let decision_aligner_tx = decision_tx.clone();
        let decision_cache_tx = decision_tx.clone();
        
        // Read chunk cache for storing consecutive chunks of raw signal for re-evaluation

        // Previous implementation with ArcMutex HashMap:
        // let read_cache: Arc<Mutex<HashMap<String, Vec<Vec<u8>>>>> = Arc::new(Mutex::new(HashMap::new()));
        // let read_cache_clone = Arc::clone(&read_cache);

        let cache: Cache<String, Vec<Vec<u8>>> = Cache::builder()
            // Max 10,000 entries
            .max_capacity(10_000)
            // Time to live (TTL): 5 minutes
            .time_to_live(std::time::Duration::from_secs(5*60))
            // Time to idle (TTI): 3 minutes
            .time_to_idle(std::time::Duration::from_secs(3*60))
            // Create the cache.
            .build();
        
        let cache_clearance = cache.clone();
        let cache_deposit = cache.clone();


        // ============
        // Uncache task
        // ============

        tokio::spawn(async move {
            
            while let Some(read_id) = cache_queue_rx.recv().await {
                // let mut read_cache  = read_cache_clone.lock().await;
                
                // This log is useful to ensure cache is cleared continuously -
                // seems to be a decent indicator of things going wrong in general
                log::info!("Reads cached: {}", cache_clearance.entry_count());

                cache_clearance.remove(&read_id).await;

            }
        
        });

        // ============================
        // Basecaller client submission
        // ============================

        tokio::spawn(async move {
            
            while let Some((read_id, channel, number, cached)) = basecall_rx.recv().await {
                                
                let channel_index = (channel - 1) as usize;
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
                
                if let Err(_) = pipeline_stdin.write_all(data.as_bytes()).await{
                    log::warn!("Failed to submit data to basecaller");
                };
            }

            Ok::<(), Status>(())
        
        });


        // ========================
        // Input stream and caching
        // ========================

        tokio::spawn(async move {

            while let Some(chunk_request) = request_stream.next().await {

                let chunk_request = match chunk_request {
                    Ok(request) => request,
                    Err(_) => {
                        log::warn!("Failed to parse request from incoming stream, continue stream...");
                        continue
                    }
                };
                
                if chunk_request.request == init_request {
                    // No action, continue and wait for data stream
                    log::info!("Waiting {} seconds for pipeline initialisation ...", &run_config_1.readuntil.init_delay);
                    tokio::time::sleep(tokio::time::Duration::new(run_config_1.readuntil.init_delay, 0)).await;
                    continue;
                }
                
                // Get the updated read and the number of chunks in the cache
                // Note: entry is significantly slower
                let cached = match cache_deposit.get(&chunk_request.id) {
                    Some(mut chunks) => {
                        chunks.push(chunk_request.data);
                        chunks
                    },
                    None => vec![chunk_request.data]
                };

                // Update cached chunks by reinserting
                cache_deposit.insert(chunk_request.id.clone(), cached.clone()).await;

                let read_id = chunk_request.id.clone();
                let num_chunks = cached.len();

                if num_chunks < run_config_1.readuntil.read_cache_min_chunks {
                    continue;
                } else if num_chunks > run_config_1.readuntil.read_cache_max_chunks {

                    if let Err(_) = decision_cache_tx.send(StreamfishResponse { 
                        channel: chunk_request.channel, 
                        number: chunk_request.number, 
                        decision: stop_decision
                    }).await {
                        log::warn!("Failed to send stop further data decision to response stream")
                    }

                    if let Err(_) = uncache_cache_tx.send(read_id.clone()).await {
                        log::warn!("Failed to send uncache message into uncache queue")
                    }

                    if run_config_1.dynamic.enabled {
                        dynamic_task_cache_chunks.insert(read_id.clone(), read_id.clone()).await;
                    }

                } else {
                    
                    // Unblock all before submission to basecaller
                    if run_config_1.readuntil.unblock_all_server {
                        
                        if let Err(_) = decision_cache_tx.send(StreamfishResponse { 
                            channel: chunk_request.channel, 
                            number: chunk_request.number, 
                            decision: unblock_decision
                        }).await {
                            log::warn!("Failed to send unblock decision to response stream")
                        }

                        if let Err(_) = uncache_cache_tx.send(read_id).await {
                            log::warn!("Failed to send uncache message into uncache queue")
                        }

                    } else {
                        if let Err(_) = basecall_tx.send((read_id, chunk_request.channel, chunk_request.number, cached)).await {
                            log::warn!("Failed to send basecaller data message into submission queue")
                        }
                    }
                }
            }

            Ok::<(), Status>(())
        });

        // ============================
        // Basecaller output processing
        // ============================
        
        tokio::spawn(async move {

            // Initiate variables for line parsing of fastq record
            let mut line_counter = 0;
            
            // We process the stream as the fastest way is dealing with each distinct line iteration
            let mut current_id: String = String::new();
            let mut current_channel: u32 = 0;
            let mut current_number: u32 = 0;
            
            while let Some(line) = pipeline_stdout.next().await {

                let line = match line {
                    Ok(line) => line,
                    Err(_) => {
                        // This needs to be more robust and allow for continuation [TODO]
                        return Err(Status::internal("Failed to parse basecaller output line"))
                    }
                };

                if line_counter == 3 {
                    // Reset line counter at end of record
                    line_counter = 0;
                } else if line_counter == 2 {
                    // Skip this line of record
                    line_counter += 1;
                } else if line_counter == 1 {
                    
                    // Unblock all after basecaller and before alignment
                    if run_config_2.readuntil.unblock_all_basecaller {
                                                
                        if let Err(_) = decision_basecaller_tx.send(StreamfishResponse { 
                            channel: current_channel, 
                            number: current_number, 
                            decision: unblock_decision
                        }).await {
                            log::warn!("Failed to send unblock decision to response stream")
                        }

                        if let Err(_) = uncache_basecaller_tx.send(current_id.clone()).await {
                            log::warn!("Failed to send uncache message to uncache queue")
                        }

                        line_counter += 1;
                        continue;
                    }

                    if let Err(_) = alignment_tx.send(
                        FastxRecord {
                            channel: current_channel,
                            number: current_number,
                            seq: line,
                            id: current_id.clone()
                        }
                    ).await {
                        log::warn!("Failed to send sequence record to alignment queue")
                    }

                    line_counter += 1;

                } else if line_counter == 0 {

                    // Parse record header to identifiers
                    let identifiers: Vec<&str> = line.split("::").collect();
                    current_id = identifiers[0].trim_start_matches("@").to_string();

                    // If we fail to parse accurate information from the identifier
                    // something has gone wrong on the basecaller output end and we
                    // terminate the client 
                    current_channel = match identifiers[1].parse::<u32>() {
                        Ok(channel) => channel,
                        Err(_) => {
                            return Err(Status::internal("Failed to parse channel from basecaller output header"))
                        }
                    };
                    current_number = match identifiers[2].parse::<u32>() {
                        Ok(number) => number,
                        Err(_) => {
                            return Err(Status::internal("Failed to parse number from basecaller output header"))
                        }
                    };

                    line_counter += 1;
                } else {
                    return Err(Status::internal("Something weird happened with the line counter..."))
                }
            }

            Ok::<(), Status>(())
        });


        // =========================
        // Alignment processing task
        // =========================
        
        tokio::spawn(async move {

            // This might silently fail if reference does not exist - run checks on config builder! [TODO]

            let aligner = match Aligner::builder().map_ont().with_index(run_config_2.experiment.reference.clone(), None) {
                Ok(aligner) => {
                    log::info!("Built the aligner for: {:?}", run_config_2.experiment.reference.display());
                    aligner
                },
                Err(_) => return Err(Status::internal(format!("Failed to build aligner from reference: {}", run_config_2.experiment.reference.display())))
            };

            // Alignment queue
            while let Some(record_data) = alignment_rx.recv().await {

                let mappings = match aligner.map(record_data.seq.as_bytes(), false, false, None, None) {
                    Ok(mappings) => mappings,
                    Err(_) => {
                        log::warn!("Failed to map read, continue with next alignment");
                        continue
                    }
                };  
                
                // DO NOT LOG THE MAPPINGS OR USE BORROWS ON THEM - FOR SOME ULTRA ARCANE REASON THIS BRAKES EVERYTHING
                // I THINK IT IS MUSL BUILD RELATED - NORMAL X86_64 BUILDS DO NOT RUN WILD EXCEPT FOR MISCONFIGURED EXPERIMENTS
                
                // Dynamic mapping updates require Arc<Mutex<MappingConfig>> locks within the
                // decision/evaluation stream- this does introduce some latency but is
                // not as much as anticipated.

                let dynamic_mapping_config = dynamic_mapping_config_stream.lock().await; 

                let decision = dynamic_mapping_config.decision_from_mapping(mappings);

                if decision == stop_decision || decision == unblock_decision {
                    if let Err(_) = uncache_aligner_tx.send(record_data.id.clone()).await {
                        log::warn!("Failed to send uncache message to uncache queue")
                    }
                }

                // On stop further data decision, add the read identifier to the 
                // dynamic mapping config cache - do we need a full cache or just
                // an ArcMutex vector? Note also this might miss the reads that received
                // a "Proceed" decision but terminated before another chunk could be sent
                // and therefore got stuck in the cache
                if run_config_2.dynamic.enabled && decision == stop_decision {
                    dynamic_task_cache_decision.insert(record_data.id.clone(), record_data.id.clone()).await;
                }


                // Unblock all after alignment and decision evaluation
                if run_config_2.readuntil.unblock_all_mapper {
        
                    if let Err(_) = decision_aligner_tx.send(StreamfishResponse { 
                        channel: record_data.channel, 
                        number: record_data.number, 
                        decision: unblock_decision
                    }).await {
                        log::warn!("Failed to send unblock decision to response stream")
                    }

                    if let Err(_) = uncache_aligner_tx.send(record_data.id).await {
                        log::warn!("Failed to send uncache message to uncache queue")
                    }

                    continue
                }

                // Otherwise send decision response
                if let Err(_) = decision_aligner_tx.send(StreamfishResponse { 
                    channel: record_data.channel, 
                    number: record_data.number, 
                    decision: decision
                }).await {
                    log::warn!("Failed to send decision to response stream")
                }
            }

            Ok::<(), Status>(())

        });


        // ==========================
        // Response stream from queue
        // ==========================

        let pipeline_response_stream = async_stream::try_stream! {
            while let Some(response) = decision_rx.recv().await {
                yield response
            }
        };

        Ok(Response::new(Box::pin(pipeline_response_stream) as Self::CacheStream))

    }
}