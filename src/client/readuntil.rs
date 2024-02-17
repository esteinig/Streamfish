
use uuid::Uuid;
use tokio::signal;
use tokio::fs::File;
use chrono::prelude::*;
use quanta::{Clock, Instant};
use tokio::io::AsyncWriteExt;
use crate::config::ServerType;
use crate::server::dori::DoriServer;
use crate::client::error::ClientError;
use crate::client::icarust::IcarustRunner;
use crate::client::minknow::MinknowClient;
use crate::utils::get_basecall_client_input;
use icarust::config::Config as IcarustConfig;
use crate::client::services::data::DataClient;
use crate::client::dori::AdaptiveSamplingClient;
use crate::services::dori_api::adaptive::Decision;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::client::services::acquisition::AcquisitionClient;
use crate::services::minknow_api::data::GetLiveReadsRequest;
use crate::config::{StreamfishConfig, Basecaller, SliceDiceConfig};
use crate::services::minknow_api::data::get_live_reads_request::action;
use crate::services::dori_api::adaptive::{RequestType, StreamfishRequest};
use crate::services::minknow_api::acquisition::{MinknowStatus, CurrentStatusRequest};


use crate::services::minknow_api::data::get_live_reads_request::{
    Actions, 
    Action, 
    StopFurtherData, 
    UnblockAction, 
    StreamSetup, 
    Request as LiveReadsRequest
};

use super::icarust::StreamfishBenchmark;

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

fn send_termination_signal(sender: &UnboundedSender<ClientErrorSignal>, error: ClientError, delay: u64) -> ClientError {
    sender.send(ClientErrorSignal {  }).map_err(|_| ClientError::ShutdownQueueSend).unwrap();

    // Termination signal has been sent to the termination thread  give it some time to execute and then terminate 
    // the function - this will bubble up and error (main errors) or respond with a configured termination response
    // from the thread joined at the end of the main routine

    log::error!("{}", error.to_string()); 

    // If we bubble up errors from the main routine, we want to give some time for the termination task to complete,
    // this is done by specifying a delay in seconds for the relevant termination signals

    if delay > 0 {
        log::info!("Delay error return ({} seconds)", &delay);
        std::thread::sleep(std::time::Duration::from_secs(delay));  // blocking async main rotune
    }   
    error
}

pub enum Termination {
    ProcessExit,
    BenchmarkError,
    SliceError,
    Error,
    Ok
}

#[derive(Debug)]
pub struct ClientErrorSignal { }

#[derive(Debug, Clone)]
pub struct ReadUntilClient { }

// Do not use Strings when it can be avoided, introduces too much latency, use str refs (&str) or enumerations
// this introducted a bit of latency into the logging as string name conversion

impl ReadUntilClient {

    pub fn new() -> Self {
        Self { }
    }

    // Run a slice-and-dice configuration
    pub async fn run_slice_dice(self, config: &StreamfishConfig, slice_dice: &SliceDiceConfig) -> Result<(), ClientError> {

        let slice_configs = slice_dice.get_configs(config);
        let client_names = slice_configs.iter().map(|x| x.meta.client_name.clone()).collect::<Vec<String>>();

        log::info!("Launching slice-and-dice configuration for adaptive sampling client:");
        log::info!("{}", &slice_dice);

        let mut task_handles = Vec::new();
        for (i, slice_cfg) in slice_configs.into_iter().enumerate() {
            
            let client = self.clone();  // clone the client

            log::info!("Launching slice runner with configuration:");
            log::info!("{}", &slice_dice.slice[i]);

            let handle = tokio::spawn(async move {
                client.run_cached(slice_cfg, None, None, Termination::SliceError).await?;  // run the slice config
                Ok::<(), ClientError>(())
            });
            task_handles.push(handle);
        }

        // Await the spawned tasks to return error and shutdown messages for each slice
        for (i, handle) in task_handles.into_iter().enumerate() {
            match handle.await {
                Ok(Ok(())) => { },
                Ok(Err(e)) => log::warn!("{}: {}", client_names[i], e.to_string()),
                Err(e) => log::error!("Join error on slice {}: {}", i, e.to_string())
            };
        };

        Ok(())
    }
    // Run a benchmark configuration
    pub async fn run_benchmark(self, benchmark: &StreamfishBenchmark, force: bool, termination: Termination) -> Result<(), ClientError> {

        let start = Utc::now();
        log::info!("Benchmark runner started at: {}", start);

        log::info!("{} ({} :: {})", benchmark.name, benchmark.date, benchmark.commit);
        log::info!("Description: {}", benchmark.description);

        log::info!("Configuring benchmarks in: {}", benchmark.outdir.display());
        log::info!("Streamfish configuration base: {}", benchmark.streamfish_config.display());
        log::info!("Icarust configuration base: {}", benchmark.icarust_config.display());

        let run_configs = benchmark.configure(force)?;
        
        log::info!("Launching benchmark loop...");
        for (group_config, benchmark_config, streamfish_config, icarust_config) in run_configs {
            log::info!("Benchmark: group={} prefix={} uuid={}", &group_config.prefix, &benchmark_config.prefix, &benchmark_config.uuid);

            let result = self.run_cached(
                streamfish_config, 
                Some(icarust_config), 
                Some(benchmark_config.uuid.clone()),
                Termination::BenchmarkError
                ).await;

            match result {
                Err(ClientError::BenchmarkTerminationError) => {
                    log::info!("Completed benchmark {}",  benchmark_config.uuid);
                    continue
                },
                Err(e) => {
                    log::error!("An error occurred in the benchmark loop: {}", e);
                    match termination {
                        Termination::BenchmarkError => return Err(e),
                        _ => std::process::exit(1)
                    }
                },
                Ok(_) => {
                    log::info!("Completed the main routine, this is unusual...");
                    continue
                }
            }

        }

        let completed = Utc::now() - start;
        log::info!(
            "Completed benchmarks in: {:0>2}m {:0>2}s",
            (completed.num_seconds() / 60) % 60, 
            completed.num_seconds() % 60
        );

        log::info!("Exiting benchmark process... ");
        std::process::exit(0);

        Ok(())
    }

    pub async fn run_cached(&self, config: StreamfishConfig, icarust_config: Option<IcarustConfig>, run_id: Option<String>, termination: Termination) -> Result<(), ClientError> {

        let (shutdown_tx, mut shutdown_rx): (UnboundedSender<ClientErrorSignal>, UnboundedReceiver<ClientErrorSignal>) = tokio::sync::mpsc::unbounded_channel();
        let (terminate_tx, mut terminate_rx) = tokio::sync::mpsc::unbounded_channel();

        let terminate_from_join_handle_tx = terminate_tx.clone();

        log::info!("{:#?}", config);

        // ============================
        // Launch Icarust if configured
        // ============================

        let icarust_task_handle = match (config.icarust.enabled, config.icarust.launch) {
            (true, true) => {

                log::info!("Creating IcarustRunner...");
                let icarust_runner = IcarustRunner::new(&config, icarust_config, run_id);

                log::info!("Icarust data delay is: {} seconds", &config.icarust.delay);
                log::info!("Icarust data runtime is: {} seconds", &config.icarust.runtime);

                let icarust_shutdown_tx = shutdown_tx.clone();

                log::info!("Launching Icarust in background task...");
                let icarust_handle = tokio::spawn(async move {
                    
                    icarust_runner.icarust.run(config.icarust.delay, config.icarust.runtime).await.map_err(|_| {
                        send_termination_signal(&icarust_shutdown_tx, ClientError::IcarustRunner, 0)
                    })?;
                    
                    log::info!("Sending termination signal after Icarust completion");
                    send_termination_signal(&icarust_shutdown_tx, ClientError::ControlServerConnectionTermination, 0);

                    Ok::<(), ClientError>(())

                });

                log::info!("Waiting for Icarust to load signal data...");
                // We should wait a little while for Icarust to load the signal data 
                tokio::time::sleep(std::time::Duration::from_secs(config.icarust.task_delay)).await;

                Some(icarust_handle)
            },
            (true, false) => {
                log::warn!("Icarust is enabled but automatic launch is disabled - manual setup required");
                None
            },
            _ => None
        };

        // Connect to control server

        let minknow_client = MinknowClient::connect(&config.minknow, Some(config.icarust.clone())).await?;

        // Wait a little just in case
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Unblock-all warnings on startup
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

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        
        // Actions sent to MinKNOW
        let stop_decision: i32 = Decision::StopData.into();
        let unblock_decision: i32 = Decision::Unblock.into();

        // Request type to Dori
        let data_request: i32 = RequestType::Data.into();
        let init_request: i32 = RequestType::Init.into();

        let run_config = config.readuntil.clone();
        let experiment_config = config.experiment.clone();

        let clock = Clock::new();


        // ==============================================
        // Launch Dori and basecall servers if configured
        // ==============================================

        let dori_dynamic_task_handle = match (config.dynamic.enabled, config.dynamic.launch_server) {
            (true, true) => {
                let dori_config = config.clone();

                log::info!("Launching Dori DynamicFeedback service as server...");
                let dori_thread_handle = tokio::spawn(async move {

                    DoriServer::run(dori_config, ServerType::Dynamic).await.map_err(|_| ClientError::DoriServerLaunch)?;

                    Ok::<(), ClientError>(())

                });

                if config.dori.dynamic.tcp_enabled {
                    log::info!("Dori DynamicFeedback server launched, available on TCP (http://{}:{})", config.dori.dynamic.tcp_host, config.dori.dynamic.tcp_port)
                } else {
                    log::info!("Dori DynamicFeedback server launched, available on UDS ({})", config.dori.dynamic.uds_path.display())
                }

                // Race condition in async slice-and-dice when launching multiple servers - wait a second
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                Some(dori_thread_handle)
            },
            (true, false) => {
                log::warn!("Dori DynamicFeedback server will not be launched - manual setup required");
                None
            },
            _ => None
        };


        let dori_adaptive_task_handle = match config.readuntil.launch_dori_server {
            false => {
                log::warn!("Dori AdaptiveSampling server will not be launched - manual setup required");
                None
            },
            true => {
                let dori_config = config.clone();

                log::info!("Launching Dori AdaptiveSampling service as server...");
                let dori_thread_handle = tokio::spawn(async move {

                    DoriServer::run(dori_config, ServerType::Adaptive).await.map_err(|_| ClientError::DoriServerLaunch)?;

                    Ok::<(), ClientError>(())

                });

                if config.dori.adaptive.tcp_enabled {
                    log::info!("Dori AdaptiveSampling server launched, available on TCP (http://{}:{})",  config.dori.adaptive.tcp_host,  config.dori.adaptive.tcp_port)
                } else {
                    log::info!("Dori AdaptiveSampling server launched, available on UDS ({})",  config.dori.adaptive.uds_path.display())
                }

                // Race condition in async slice-and-dice when launching multiple servers - wait a second
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                Some(dori_thread_handle)
            },
        };

        let basecaller_process_handle = match (config.readuntil.launch_basecall_server, config.dori.adaptive.basecaller.clone() ) {
            (false, _) => {
                log::warn!("Basecall server will not be launched - manual setup required");
                None
            },
            (true, Basecaller::Guppy) => {
                
                log::info!("Launching basecall server in process thread...");
                
                let process_stderr = std::process::Stdio::from(std::fs::File::create( config.basecaller.server.stderr_log.clone()).expect(
                    &format!("Failed to create basecall server log file: {}", config.basecaller.server.stderr_log.display())
                ));

                let process = std::process::Command::new( config.basecaller.server.path.as_os_str())
                    .args(config.basecaller.server.args.clone())
                    .stdout(std::process::Stdio::null())
                    .stderr(process_stderr)
                    .spawn()
                    .expect("Failed to spawn basecall server process");
                
                log::info!("Basecall server launched, available on: {}",  config.basecaller.client.address);

                // Wait a little due to race condition in async slice-and-dice
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                Some(process)

            },
            _ => {
                unimplemented!("Basecaller not implemented")
            }
        };


        // ===========================================
        // Single chunks reference writer [optional]
        // ===========================================
        
        let (chunk_writer_tx, mut chunk_writer_rx) = tokio::sync::mpsc::unbounded_channel();
        let write_chunk_file = !run_config.unblock_all_chunk_file.as_os_str().is_empty();
        let shutdown_error_chunk_writer_tx = shutdown_tx.clone();

        // Only need this for optional chunk writer - might be good getting this on client 
        // rather than RPC server endpoint and send with initialisation request?

        let chunk_writer_handle = match write_chunk_file {
            true => {
                // Calibration data - accounts for manual setting of sample rate in Icarust
                let (sample_rate, calibration) = minknow_client.get_device_data(
                    &config.readuntil.device_name, &1, &config.readuntil.channels  // Note that we get the whole channel array even if we slice
                ).await.map_err(|_| {
                    send_termination_signal(&shutdown_error_chunk_writer_tx, ClientError::ControlServerDeviceCalibration, 5)
                })?;

                let chunk_writer_handle = tokio::spawn(async move {

                        let mut output = File::create(&run_config.unblock_all_chunk_file).await.map_err(|_| {
                            send_termination_signal(&shutdown_error_chunk_writer_tx, ClientError::ChunkFileCreate, 0)
                        })?;

                        while let Some((read_id, channel, number, chunk)) = chunk_writer_rx.recv().await {
                                            
                            let channel_index = (channel - 1) as usize;
                        
                            output.write_all(get_basecall_client_input(
                                read_id, 
                                chunk,
                                1, 
                                channel, 
                                number,
                                calibration.offsets[channel_index],
                                calibration.pa_ranges[channel_index], 
                                calibration.digitisation, 
                                sample_rate
                            ).as_bytes()).await.map_err(|_| {
                                send_termination_signal(&shutdown_error_chunk_writer_tx, ClientError::ChunkWriterQueueSend, 0)
                            })?;
                            
                        }

                        Ok::<(), ClientError>(())
                    
                });

                Some(chunk_writer_handle)
            }
            false => None
        };


        // ==========================================
        // Process clean-up task + terminate signal
        // ==========================================


        let _ = tokio::spawn(async move {

            let mut manual_shutdown = false;

            // Shutdown of threads and server tasks 
            tokio::select! {
                _ = signal::ctrl_c() => {
                    log::warn!("Received manual shutdown signal");
                    manual_shutdown = true;
                },
                _ = shutdown_rx.recv() => log::warn!("Received shutdown signal"),
            }
            
            if let Some(mut basecaller_thread) = basecaller_process_handle {
                log::warn!("Shutting down basecall server process...");

                basecaller_thread.kill().map_err(|err| err).expect("Failed to kill basecaller thread - you may need to do this manually");
            }



            if let Some(writer_task) = chunk_writer_handle {
                log::warn!("Shutting down chunk writer...");
                writer_task.abort();
            }

            if let Some(dori_task) = dori_adaptive_task_handle {
                log::warn!("Shutting down adaptive sampling server...");
                dori_task.abort();
            }

            if let Some(dori_task) = dori_dynamic_task_handle {
                log::warn!("Shutting down dynamic feedback server...");
                dori_task.abort();
            }

            if let Some(icarust_task) = icarust_task_handle {
                log::warn!("Shutting down Icarust runner...");
                icarust_task.abort();
            }

            log::warn!("Completed task shutdowns, terminate client");
            log::info!("So long, and thanks for all the fish... 🐟 🐟 🐟");

            if manual_shutdown {
                // Terminate the process
                std::process::exit(1)
            } else {
                // Otherwise send the termination signal to the main routine 
                // in case the termination configurations asks for return from this method
                terminate_tx.send(true).expect("Failed to send termination signal to main routine");
            }
        });


        


        
        // ==========================================
        // MPSC message queues: senders and receivers
        // ==========================================

        // TODO: check if better to have bounds
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel();
        let (dori_tx, mut dori_rx) = tokio::sync::mpsc::unbounded_channel();
        let (log_tx, mut log_rx) = tokio::sync::mpsc::unbounded_channel();
        let (throttle_tx, mut throttle_rx) = tokio::sync::mpsc::unbounded_channel();


        // Clocks and sender clones
        let minknow_response_log = log_tx.clone();
        let minknow_response_clock = clock.clone();

        let dori_response_log = log_tx.clone();
        let dori_response_clock = clock.clone();
        
        let minknow_dori_action_tx = action_tx.clone();
        let minknow_action_tx = action_tx.clone(); 
        let dori_data_tx = dori_tx.clone(); 



        // ===================
        // Client connections
        // ===================

        let mut dori_rpc = AdaptiveSamplingClient::connect(&config).await.map_err(|_| {
            send_termination_signal(&shutdown_tx, ClientError::DoriServerConnectionInitiation, 5)
        })?;

        let mut data_rpc = DataClient::from_minknow_client(
            &minknow_client, &run_config.device_name
        ).await.map_err(|err| {
            send_termination_signal(&shutdown_tx, err, 5)
        })?;


        let mut acquisition_rpc = AcquisitionClient::from_minknow_client(
            &minknow_client, &run_config.device_name
        ).await.map_err(|err| {
            send_termination_signal(&shutdown_tx, err, 5)
        })?;


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

            let request = tonic::Request::new(CurrentStatusRequest {});
            let response = acquisition_rpc.client.current_status(request).await.map_err(|_| {
                send_termination_signal(&shutdown_tx, ClientError::ControlServerAcquisitionStatusRequest, 5)
            })?.into_inner();

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
                accepted_first_chunk_classifications: run_config.accepted_first_chunk_classifications.clone(), 
                max_unblock_read_length: None
            }))
        };

        // Send it into the action queue that unpacks into the request stream - this must happen before the request to control server weirdly
        action_tx.send(init_action).map_err(|_| {
            send_termination_signal(&shutdown_tx, ClientError::ControlServerStreamInitSend, 5)
        })?;

        // DataService response stream is initiated with the data request stream to MinKNOW
        let mut minknow_stream = data_rpc.client.get_live_reads(tonic::Request::new(data_request_stream)).await.map_err(|_| {
            send_termination_signal(&shutdown_tx, ClientError::ControlServerStreamInit, 5)
        })?.into_inner();

        log::info!("Initiated data streams with control server");

        // Initiate processing server stream - must happpen before the request to the processing server weirdly
        dori_tx.send(StreamfishRequest { channel: 0, number: 0, id: String::new(), data: Vec::new(), request: init_request }).map_err(|_| {
            send_termination_signal(&shutdown_tx, ClientError::DoriServerStreamInitSend, 5)
        })?;

        let mut dori_stream = dori_rpc.client.cache(tonic::Request::new(dori_request_stream)).await.map_err(|_| {
            send_termination_signal(&shutdown_tx, ClientError::DoriServerStreamInit, 5)
        })?.into_inner();


        // ===========================================
        // Main adaptive sampling routine is initiated
        // ===========================================


        log::info!("Initiated data streams with Dori ({}s delay)", &run_config.init_delay);
        tokio::time::sleep(tokio::time::Duration::new(run_config.init_delay, 0)).await;

        let start = clock.now();
        log::info!("Started adaptive sampling loops...");

        // ========================================
        // DataService response stream is processed
        // ========================================

        let control_server_response_error_tx = shutdown_tx.clone();
        let action_stream_handle = tokio::spawn(async move {



            while let Some(response) = minknow_stream.message().await.map_err(|_| {
                send_termination_signal(&control_server_response_error_tx, ClientError::ControlServerConnectionTermination, 0)
            })?

            {
                for (channel, read_data) in response.channels {
                    if run_config.unblock_all_client {
                            // Unblock all configuration to test client-side unblocking 
                            minknow_action_tx.send(GetLiveReadsRequest { request: Some(
                                LiveReadsRequest::Actions(Actions { actions: vec![
                                    Action {
                                        action_id: Uuid::new_v4().to_string(), // Check if this is really costly?
                                        read: Some(action::Read::Number(read_data.number)),
                                        action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                                        channel: channel,
                                    }
                                ]})
                            )}).map_err(|_| {
                                send_termination_signal(&control_server_response_error_tx, ClientError::ControlServerActionQueueSend, 0) 
                            })?;

                            // If chunk file is configured, write the chunk to file
                            if write_chunk_file {
                                chunk_writer_tx.send((read_data.id, channel, read_data.number, read_data.raw_data)).map_err(|_| {
                                    send_termination_signal(&control_server_response_error_tx, ClientError::DoriStreamQueueSend, 0)
                                })?;
                            }

                    } else {
                    
                        // Sends single channel data to Dori
                        dori_data_tx.send(StreamfishRequest {
                            id: read_data.id,
                            channel: channel,
                            number: read_data.number,
                            data: read_data.raw_data,
                            request: data_request
                        }).map_err(|_| {
                            send_termination_signal(&control_server_response_error_tx, ClientError::DoriStreamQueueSend, 0)
                        })?;
                         
                    }

                    minknow_response_log.send(ClientLog { 
                        channel, 
                        number: read_data.number,
                        stage: PipelineStage::DoriRequest, 
                        time: minknow_response_clock.now(),
                    }).map_err(|_| {
                        send_termination_signal(&control_server_response_error_tx, ClientError::LoggingQueueSend, 0)
                    })?;
                }

            }

            Ok::<(), ClientError>(())
        });
        
        // ========================================
        // DoriService response stream is processed
        // ========================================


        let dori_response_error_tx = shutdown_tx.clone();
        let dori_stream_handle = tokio::spawn(async move {

            while let Some(dori_response) = dori_stream.message().await.map_err(|_| {
                send_termination_signal(&dori_response_error_tx, ClientError::DoriServerConnectionTermination, 0)
            })?

            {
                if experiment_config.control {
                    continue;
                }

                if dori_response.decision == unblock_decision {
                    // Send unblock decision to stop read - also stops further data (minknow_api::data)
                    throttle_tx.send(
                        Action {
                            action_id: Uuid::new_v4().to_string(),
                            read: Some(action::Read::Number(dori_response.number)),
                            action: Some(action::Action::Unblock(UnblockAction { duration: run_config.unblock_duration })),
                            channel: dori_response.channel,
                        }
                    ).map_err(|_| {
                        send_termination_signal(&dori_response_error_tx, ClientError::DecisionQueueSend, 0)
                    })?;
                    

                } else if dori_response.decision == stop_decision {

                    throttle_tx.send(
                        Action {
                            action_id: Uuid::new_v4().to_string(),
                            read: Some(action::Read::Number(dori_response.number)),
                            action: Some(action::Action::StopFurtherData(StopFurtherData {})),
                            channel: dori_response.channel,
                        }
                    ).map_err(|_| {
                        send_termination_signal(&dori_response_error_tx, ClientError::DecisionQueueSend, 0)
                    })?;

                } else {
                    // Sends a none action - may not be needed, could use `continue`
                    throttle_tx.send(
                        Action {
                            action_id: Uuid::new_v4().to_string(),
                            read: Some(action::Read::Number(dori_response.number)),
                            action: None,
                            channel: dori_response.channel,
                        }
                    ).map_err(|_| {
                        send_termination_signal(&dori_response_error_tx, ClientError::DecisionQueueSend, 0)
                    })?;

                }

                // Always send a log entry to logging thread
                dori_response_log.send(ClientLog { 
                    stage: PipelineStage::DoriResponse, 
                    time: dori_response_clock.now(), 
                    channel: dori_response.channel, 
                    number: dori_response.number 
                }).map_err(|_| {
                    send_termination_signal(&dori_response_error_tx, ClientError::LoggingQueueSend, 0)
                })?;

            }

            Ok::<(), ClientError>(())
        });


        let throttle_error_tx = shutdown_tx.clone();
        let throttle_handle = tokio::spawn(async move {

            let throttle = std::time::Duration::from_millis(run_config.action_throttle);
            let mut actions = Vec::new();
            let mut t0 = clock.now();

            while let Some(control_action) = throttle_rx.recv().await {

                // Streaming mode - without additional time or logic control overhead (minimal)
                if run_config.action_throttle == 0 {
                    minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                        LiveReadsRequest::Actions(Actions { actions: Vec::from([control_action]) })
                    )}).map_err(|_| {
                        send_termination_signal(&throttle_error_tx, ClientError::ControlServerActionQueueSend, 0)
                    })?;

                    continue;
                }
                
                let t1 = clock.now();
                actions.push(control_action);
                                
                if t1 - throttle >= t0 {

                    minknow_dori_action_tx.send(GetLiveReadsRequest { request: Some(
                        LiveReadsRequest::Actions(Actions { actions })
                    )}).map_err(|_| {
                        send_termination_signal(&throttle_error_tx, ClientError::ControlServerActionQueueSend, 0)
                    })?;

                    actions = Vec::new();
                    t0 = clock.now();
                }
            }

            Ok::<(), ClientError>(())
        });

        let log_error_tx = shutdown_tx.clone();
        let logging_handle = tokio::spawn(async move {

            // Routine when specifing a log file in configuration:
            if let Some(path) = run_config.latency_log {

                let mut log_file = File::create(&path).await.map_err(|_| {
                    send_termination_signal(&log_error_tx, ClientError::LogFileCreate, 0)
                })?;

                while let Some(log) = log_rx.recv().await {
                    log::debug!("{:<15} {:<5} {:<7}", log.stage.as_str_name(), log.channel, log.number);

                    log_file.write_all(log.entry(start).as_bytes()).await.map_err(|_| {
                        send_termination_signal(&log_error_tx, ClientError::LogFileWrite, 0)
                    })?;
                }
                
            } else {
                // Otherwise log to console
                while let Some(log) = log_rx.recv().await {
                    log::debug!("{:<15} {:<5} {:<7}", log.stage.as_str_name(), log.channel, log.number);
                }
            }
            
            Ok::<(), ClientError>(())
        });

        // ===================================
        // Await thread handles to run streams
        // ===================================

        
        let stream_join_handle = tokio::spawn(async move {
            
            // This runs the stream network - not quite clear if we actually
            // need to await these handles for the core stream functions,
            // we never see the awaited stream or join error
            for handle in [
                action_stream_handle,
                dori_stream_handle,
                logging_handle,
                throttle_handle,
            ] 
            {
                match handle.await {
                    Ok(Ok(())) => {
                        log::error!("Task has been joined... this shouldn't happen since we are running streams");
                    },
                    Ok(Err(e)) => {
                        log::error!("Stream error: {}", e.to_string());
                        terminate_from_join_handle_tx.send(true).expect("Failed to send termination signal to main routine");
                    },
                    Err(e) => {
                        log::error!("Join error: {}", e.to_string());
                        terminate_from_join_handle_tx.send(true).expect("Failed to send termination signal to main routine");
                    } 
                }
            }  

        });


        if let Some(_) = terminate_rx.recv().await {

            stream_join_handle.abort();

            // Wait a little while for things to settle
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            match termination {
                Termination::ProcessExit => std::process::exit(1),
                Termination::BenchmarkError => return Err(ClientError::BenchmarkTerminationError),
                Termination::SliceError => return Err(ClientError::SliceTerminationError),
                Termination::Error => { }
                Termination::Ok => { }
            }

        }
        
        Ok(())
    }
}

