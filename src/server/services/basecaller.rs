
use crate::client::minknow::MinKnowClient;
use crate::config::StreamfishConfig;
use crate::services::dori_api::basecaller::basecaller_server::Basecaller;
use crate::services::dori_api::basecaller::{BasecallerRequest, BasecallerResponse, basecaller_response::Decision};

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

pub struct BasecallerService {
    config: StreamfishConfig
}
impl BasecallerService {
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
impl Basecaller for BasecallerService {

    type BasecallDoradoStream = Pin<Box<dyn Stream<Item = Result<BasecallerResponse, Status>> + Send  + 'static>>;

    // Basecalling with Dorado RPC
    async fn basecall_dorado(&self, request: Request<tonic::Streaming<BasecallerRequest>>) -> Result<Response<Self::BasecallDoradoStream>, Status> {
        
        log::info!("Initiated Dori::BasecallerService::BasecallDorado RPC");

        // Used in generators, needs distinct clones
        let run_config_1 = self.config.clone();
        let run_config_2 = self.config.clone();

        // Define the decisions as <i32> - repeated into() calls in the stream processing
        // loops introduce a tiny bit of latency! Make sure calls like this are minimized.
        let none_decision: i32 = Decision::None.into();
        let continue_decision: i32 = Decision::Continue.into();
        let unblock_decision: i32 = Decision::Unblock.into();

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
        let mut channel_caches: Vec<HashMap<u32, Vec<BasecallerRequest>>> = Vec::new();

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

                let channel = dorado_request.channel.clone();
                let number = dorado_request.number.clone();

                let channel_index = (dorado_request.channel-1) as usize;

                // Use this to generate a test dataset to pipe into Dorado STDIN

                // println!("{}", dorado_request.to_stdin(
                //     calibration.offsets[channel_index],
                //     calibration.pa_ranges[channel_index],
                //     calibration.digitisation,
                //     sample_rate
                // ).trim());
                

                // Introduces little bit of latency when un ublock-all mode (1-5 bp)
                // right now there are no limits or cache removals - this means the
                // cache will grow massively as reads are streaming in
                let read_cache = &mut channel_caches[channel_index];
                
                // Find cached read or insert new one, update the cache with the current read
                read_cache.entry(dorado_request.number).or_insert(Vec::new()).push(dorado_request);

                // Get the updated read and concat all chunks to data strings to send as bytes to Dorado
                let cached = read_cache.get(&number).expect("Failed to get data from channel cache");
                
                let data: Vec<String> = cached.iter().map(|req| {
                    req.to_stdin(
                        calibration.offsets[channel_index],
                        calibration.pa_ranges[channel_index],
                        calibration.digitisation,
                        sample_rate
                    )
                }).collect();

                pipeline_stdin.write_all(data.concat().as_bytes()).await?;

                let response = BasecallerResponse { 
                    channel, 
                    number,
                    decision: match run_config_1.readuntil.unblock_all_dori { true => unblock_decision, false => none_decision }
                };
                yield response
            }
        };
        
        let test = true;
        let pipeline_output_response_stream = async_stream::try_stream! {

            while let Some(line) = pipeline_stdout.next().await {
                let line = line?;
                if line.starts_with('@') { 
                    continue; 
                } else {
                    match test {
                        true => yield process_dorado_read_sam(&line, &run_config_2, unblock_decision, continue_decision).await,
                        false => yield process_dorado_read_sam(&line, &run_config_2, unblock_decision, continue_decision).await
                    }
                }
            }
        };

        let response_stream_1 = futures::stream::select(
            pipeline_input_response_stream,
            pipeline_output_response_stream, 
        );

        Ok(Response::new(Box::pin(response_stream_1) as Self::BasecallDoradoStream))
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
async fn process_dorado_read_sam(line: &str, config: &StreamfishConfig, unblock_decision: i32, continue_decision: i32) -> BasecallerResponse {
                
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

        BasecallerResponse { 
            channel: identifiers[1].parse::<u32>().unwrap(), 
            number: identifiers[2].parse::<u32>().unwrap(), 
            decision
        }
}


use byteorder::{LittleEndian, ByteOrder};

impl BasecallerRequest {
    pub fn to_stdin(&self, offset: f32, range: f32, digitisation: u32, sample_rate: u32) -> String {

        // UNCALBIRATED SIGNAL CONVERSON BYTES TO SIGNED INTEGERS
        let mut signal_data: Vec<i16> = Vec::new();
        for i in (0..self.data.len()).step_by(2) {
            signal_data.push(LittleEndian::read_i16(&self.data[i..]));
        }

        // TODO CHECK EFFECTS: Float formatting precision - POD5 inspection suggest 13 digits for range (PYTHON) - not sure if necessary
        format!("{} {} {} {} {:.1} {:.11} {} {}\n", self.id, self.channel, self.number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
    }
}


        // let basecaller2_stderr = Stdio::from(std::fs::File::create("basecaller2.err").expect("Failed to create basecaller stderr file: basecaller.err"));

        // let mut basecaller2_process = Command::new(self.config.dori.dorado_path.as_os_str())
        //     .args(&self.config.dori.dorado_args)
        //     .stdin(Stdio::piped())
        //     .stdout(Stdio::piped())
        //     .stderr(basecaller2_stderr)
        //     .spawn()
        //     .expect("Failed to spawn basecaller process");

        // let mut basecaller2_stdin = basecaller2_process.stdin.take().expect("Failed to open basecaller STDIN");
        // let basecaller2_stdout = basecaller2_process.stdout.take().expect("Failed to open basecaller STDOUT");

        // let mut basecaller2_lines = BufReader::new(basecaller2_stdout).lines();

        // Classification stage - at the moment just the same outputs with the minimal basecaller
        // input/output program to measure effects on latency
        
        
        // let classifier_stderr = Stdio::from(std::fs::File::create("classifier.err").expect("Failed to create basecaller stderr file: classify.err"));

        // let mut classifier_process = Command::new(self.config.dori.classifier_path.as_os_str())
        //     .args(&self.config.dori.classifier_args)
        //     .stdin(Stdio::piped())
        //     .stdout(Stdio::piped())
        //     .stderr(classifier_stderr)
        //     .spawn()
        //     .expect("Failed to spawn basecaller process");

        // let mut classifier_stdin = classifier_process.stdin.take().expect("Failed to open basecaller STDIN");
        // let classifier_stdout = classifier_process.stdout.take().expect("Failed to open basecaller STDOUT");

        // let mut classifier_lines = BufReader::new(classifier_stdout).lines();



        // Write FASTQ into the classifier test script
        // classifier_stdin.write_all(line.as_bytes()).await?;  // return is stripped
        // classifier_stdin.write_all("\n".as_bytes()).await?;

        // if fq_line == 1 {
        //     let content: Vec<&str> = line.trim().split("::").collect();

        //     log::info!("{}", line);
        //     yield  BasecallerResponse { 
        //         channel: content[1].parse::<u32>().unwrap(), 
        //         number: content[2].parse::<u32>().unwrap(), 
        //         id: "stdout-basecaller".to_string(),
        //         stage: PipelineStage::ClassifierOutput.into() // test for unblock
        //     };
        //     fq_line += 1;

        // } else {
        //     if fq_line == 4 { fq_line = 1 } else { fq_line += 1 }
        //     continue
        // }


        // ~ 395 bp N50  with alignment

        // tokio::spawn(async move {
        //     let mut i = 0;
        //     while let Some(dorado_request) = request_stream.next().await {
        //         let dorado_request = dorado_request.expect("Failed to parse request from ReadUntilClient");
        //         let channel_index = (dorado_request.channel-1) as usize;

        //         // Use this to generate a test dataset to pipe into Dorado STDIN

        //         // println!("{}", dorado_request.to_stdin(
        //         //     calibration.offsets[channel_index],
        //         //     calibration.pa_ranges[channel_index],
        //         //     calibration.digitisation,
        //         //     sample_rate
        //         // ).trim());
                
        //         // log::info!("{}", i);

        //         match i {
        //             0 => {
        //                 basecaller1_stdin.write_all(
        //                     dorado_request.to_stdin(
        //                         calibration.offsets[channel_index],
        //                         calibration.pa_ranges[channel_index],
        //                         calibration.digitisation,
        //                         sample_rate
        //                     ).as_bytes()
        //                 ).await.expect("Failed to write to basecaller 1");
        //                 i = 1;
        //             },
        //             1 => {
        //                 basecaller2_stdin.write_all(
        //                     dorado_request.to_stdin(
        //                         calibration.offsets[channel_index],
        //                         calibration.pa_ranges[channel_index],
        //                         calibration.digitisation,
        //                         sample_rate
        //                     ).as_bytes()
        //                 ).await.expect("Failed to write to basecaller 2");
        //                 i = 0;
        //             },
        //             _ => panic!("Iteration counter fucked up - this should not occur!")
        //         }
        //     }

        // });

        
        //  ~ 395 bp N50 with alignment

        // let basecaller_input_response_stream = async_stream::try_stream! {

        //     let mut i = 0;
        //     while let Some(dorado_request) = request_stream.next().await {
        //         let dorado_request = dorado_request?;
        //         let channel_index = (dorado_request.channel-1) as usize;

        //         // Use this to generate a test dataset to pipe into Dorado STDIN

        //         // println!("{}", dorado_request.to_stdin(
        //         //     calibration.offsets[channel_index],
        //         //     calibration.pa_ranges[channel_index],
        //         //     calibration.digitisation,
        //         //     sample_rate
        //         // ).trim());
                
        //         match i {
        //             0 => {
        //                 basecaller1_stdin.write_all(
        //                     dorado_request.to_stdin(
        //                         calibration.offsets[channel_index],
        //                         calibration.pa_ranges[channel_index],
        //                         calibration.digitisation,
        //                         sample_rate
        //                     ).as_bytes()
        //                 ).await?;
        //                 i = 1;
        //             },
        //             1 => {
        //                 basecaller2_stdin.write_all(
        //                     dorado_request.to_stdin(
        //                         calibration.offsets[channel_index],
        //                         calibration.pa_ranges[channel_index],
        //                         calibration.digitisation,
        //                         sample_rate
        //                     ).as_bytes()
        //                 ).await?;
        //                 i = 0;
        //             },
        //             _ => panic!("Iteration counter fucked up - this should not occur!")
        //         }
               


        //         let response = BasecallerResponse { 
        //             channel: 0, 
        //             number: 0, 
        //             id: "stdin-basecaller".to_string(),
        //             stage: PipelineStage::BasecallerInput.into()
        //         };
        //         yield response
        //     }
        // };
        
        // Try placing the response streams into a thread -> queue -> stream construct - maybe these actually 
        // execute in sequence?

        // let basecaller2_output_response_stream = async_stream::try_stream! {

        //     // let mut fq_line = 1;
        //     while let Some(line) = basecaller2_lines.next().await {
        //         let line = line?;
                
        //         if line.starts_with('@'){
        //             continue;
        //         }

        //         // Write FASTQ into the classifier test script
        //         // classifier_stdin.write_all(line.as_bytes()).await?;  // return is stripped
        //         // classifier_stdin.write_all("\n".as_bytes()).await?;

        //         // if fq_line == 1 {
        //         //     let content: Vec<&str> = line.trim().split("::").collect();
    
        //         //     log::info!("{}", line);
        //         //     yield  BasecallerResponse { 
        //         //         channel: content[1].parse::<u32>().unwrap(), 
        //         //         number: content[2].parse::<u32>().unwrap(), 
        //         //         id: "stdout-basecaller".to_string(),
        //         //         stage: PipelineStage::ClassifierOutput.into() // test for unblock
        //         //     };
        //         //     fq_line += 1;

        //         // } else {
        //         //     if fq_line == 4 { fq_line = 1 } else { fq_line += 1 }
        //         //     continue
        //         // }
        //         let content: Vec<&str> = line.split('\t').collect();
        //         let identifiers: Vec<&str> = content[0].split("::").collect();

        //         log::info!("Basecaller2: {}", &identifiers[0][0..8]);

        //         yield BasecallerResponse { 
        //             channel: identifiers[1].parse::<u32>().unwrap(), 
        //             number:  identifiers[2].parse::<u32>().unwrap(), 
        //             id:  identifiers[0].to_string(),
        //             stage: PipelineStage::ClassifierOutput.into()
        //         };
        //     }
        // };
        // let classifier_output_response_stream = async_stream::try_stream! {
            
        //     // let mut fq_line = 1;
        //     while let Some(line) = classifier_lines.next().await {
        //         let line = line?;

        //         // log::info!("FROM CLASSIFIER: {}", line);
                
        //         let content: Vec<&str> = line.trim().split_whitespace().collect();
    
        //         yield  BasecallerResponse { 
        //             channel: content[1].parse::<u32>().unwrap(), 
        //             number: content[2].parse::<u32>().unwrap(), 
        //             id: content[0].to_string(),
        //             stage: PipelineStage::ClassifierOutput.into()
        //         };

        //         // if fq_line == 1 {
        //         //     let content: Vec<&str> = line.trim().split("::").collect();
    
        //         //     yield  BasecallerResponse { 
        //         //         channel: content[1].parse::<u32>().unwrap(), 
        //         //         number: content[2].parse::<u32>().unwrap(), 
        //         //         id: "stdout-classifier".to_string(),
        //         //         stage: PipelineStage::ClassifierOutput.into()
        //         //     };

        //         // } else {
        //         //     if fq_line == 4 { fq_line = 0 } else { fq_line += 1 }
        //         //     continue
        //         // }

                
        //     }
        // };