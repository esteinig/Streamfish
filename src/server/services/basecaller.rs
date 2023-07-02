
use crate::client::minknow::MinKnowClient;
use crate::client::services::device::DeviceClient;
use crate::config::StreamfishConfig;
use crate::services::dori_api::basecaller::basecaller_server::Basecaller;
use crate::services::dori_api::basecaller::{BasecallerRequest, BasecallerResponse, basecaller_response::PipelineStage};

use colored::*;
use std::pin::Pin;
use futures_core::Stream;
use futures_util::StreamExt;
use futures_util::io::{BufReader, Lines};
use futures_util::AsyncWriteExt;
use futures_util::io::AsyncBufReadExt;
use tonic::{Request, Response, Status};
use async_process::{Command, Stdio, ChildStdout};
use itertools::join;

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

    type BasecallDoradoStream = Pin<Box<dyn Stream<Item = Result<BasecallerResponse, Status>> + Send  + 'static>>;

    async fn basecall_dorado( 
        &self,
        request: Request<tonic::Streaming<BasecallerRequest>>,
    ) -> Result<Response<Self::BasecallDoradoStream>, Status> {
        
        log::info!("Initiated Dori::BasecallerService::BasecallDorado on request stream connection");

        let minknow_client = MinKnowClient::connect(&self.config.minknow).await.expect("Failed to connect to MinKNOW from Dori");

        let mut device_client = DeviceClient::from_minknow_client(
            &minknow_client, &self.config.readuntil.device_name
        ).await.expect("Failed to connect to initiate device client on Dori");

        let sample_rate = device_client.get_sample_rate().await.expect("Failed to get sample rate from MinKNOW on Dori");
        let calibration = device_client.get_calibration(&self.config.readuntil.channel_start, &self.config.readuntil.channel_end).await.expect("Failed to connect to get calibration from MinKNOW on Dori");

        log::info!("Sample rate: {} | Digitisation: {}", sample_rate, calibration.digitisation);

        // Ok does this mean we can actually get the whole stream from here??



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

        // Basecall stage
        
        // Not sure if this is correct? I think so?
        let basecaller1_stderr = Stdio::from(std::fs::File::create("basecaller1.err").expect("Failed to create basecaller stderr file: basecaller.err"));

        let mut basecaller1_process = Command::new(self.config.dori.dorado_path.as_os_str())
            .args(&self.config.dori.dorado_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(basecaller1_stderr)
            .spawn()
            .expect("Failed to spawn basecaller process");

        let mut basecaller1_stdin = basecaller1_process.stdin.take().expect("Failed to open basecaller STDIN");
        let basecaller1_stdout = basecaller1_process.stdout.take().expect("Failed to open basecaller STDOUT");

        let mut basecaller1_lines = BufReader::new(basecaller1_stdout).lines();

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


        // Parsing request stream for basecaller input

        let mut request_stream = request.into_inner();

        // ~ 395 bp N50 

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

        
        //  ~ 395 bp N50 

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

        let basecaller_input_response_stream = async_stream::try_stream! {

            while let Some(dorado_request) = request_stream.next().await {
                let dorado_request = dorado_request?;
                let channel_index = (dorado_request.channel-1) as usize;

                // Use this to generate a test dataset to pipe into Dorado STDIN

                // println!("{}", dorado_request.to_stdin(
                //     calibration.offsets[channel_index],
                //     calibration.pa_ranges[channel_index],
                //     calibration.digitisation,
                //     sample_rate
                // ).trim());
                
                basecaller1_stdin.write_all(
                    dorado_request.to_stdin(
                        calibration.offsets[channel_index],
                        calibration.pa_ranges[channel_index],
                        calibration.digitisation,
                        sample_rate
                    ).as_bytes()
                ).await?;


                let response = BasecallerResponse { 
                    channel: 0, 
                    number: 0, 
                    id: "stdin-basecaller".to_string(),
                    stage: PipelineStage::BasecallerInput.into()
                };
                yield response
            }
        };
        
        let basecaller1_output_response_stream = async_stream::try_stream! {

            // let mut fq_line = 1;
            while let Some(line) = basecaller1_lines.next().await {
                let line = line?;
                
                if line.starts_with('@'){
                    continue;
                }

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
                let content: Vec<&str> = line.split('\t').collect();
                let identifiers: Vec<&str> = content[0].split("::").collect();
                let alignment_flag: &str = content[1];

                log::info!("Dorado: {} {}", &identifiers[0][0..8], match alignment_flag {  
                    "4" => "unmapped".red(), 
                    "0" => "mapped".green(), 
                    "16" => "mapped".green(), 
                    "256" => "mapped".green(),
                     _ => alignment_flag.white() 
                });

                yield BasecallerResponse { 
                    channel: identifiers[1].parse::<u32>().unwrap(), 
                    number:  identifiers[2].parse::<u32>().unwrap(), 
                    id:  identifiers[0].to_string(),
                    stage: PipelineStage::ClassifierOutput.into()
                };
            }
        };

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

        // Merge output streams - this producees no guaranteed order on stage responses

        // For some reason `futures::stream::select_all` does not work - complains about mismatch
        // between the result async stream type, showing the same type?
        //
        // Might need to report this to async_stream crate, this looks a bit inefficient
        let response_stream_1 = futures::stream::select(
            basecaller1_output_response_stream, 
            basecaller_input_response_stream,
        );
        // let response_stream_2 = futures::stream::select(
        //     response_stream_1,
        //     basecaller_input_response_stream,
        // );

        Ok(Response::new(Box::pin(response_stream_1) as Self::BasecallDoradoStream))
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

        // TODO: Float formatting precision - CHECK EFFECTS - POD5 inspection suggest 13 digits for range (PYTHON) - not sure if necessary to be precise
        format!("{} {} {} {} {:.1} {:.11} {} {}\n", self.id, self.channel, self.number, digitisation, offset, range, sample_rate, join(&signal_data, " ")) 
    }
}