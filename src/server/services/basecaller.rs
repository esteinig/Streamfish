
use crate::client::minknow::MinKnowClient;
use crate::client::services::device::DeviceClient;
use crate::config::{DoriConfig, StreamfishConfig};
use crate::services::dori_api::basecaller::basecaller_server::Basecaller;
use crate::services::dori_api::basecaller::{BasecallerRequest, BasecallerResponse, basecaller_response::PipelineStage};

use std::pin::Pin;
// use std::io::{Read, BufRead, BufReader, Write};
use futures_core::Stream;
use futures_util::StreamExt;
use futures_util::io::BufReader;
use futures_util::AsyncWriteExt;
use futures_util::io::AsyncBufReadExt;
use tonic::{Request, Response, Status};
// use std::process::{Command, Stdio};
use async_process::{Command, Stdio};
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

        let mut minknow_client = MinKnowClient::connect(&self.config.minknow).await.expect("Failed to connect to MinKNOW from Dori");

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

        let mut basecaller_process = Command::new(self.config.dori.dorado_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn basecaller process");

        let mut basecaller_stdin = basecaller_process.stdin.take().expect("Failed to open basecaller STDIN");
        let basecaller_stdout = basecaller_process.stdout.take().expect("Failed to open basecaller STDOUT");

        let mut basecaller_lines = BufReader::new(basecaller_stdout).lines();

        // Classification stage - at the moment just the same outputs with the minimal basecaller
        // input/output program to measure effects on latency

        let mut classifier_process = Command::new(self.config.dori.dorado_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn basecaller process");

        let mut classifier_stdin = classifier_process.stdin.take().expect("Failed to open basecaller STDIN");
        let classifier_stdout = classifier_process.stdout.take().expect("Failed to open basecaller STDOUT");

        let mut classifier_lines = BufReader::new(classifier_stdout).lines();


        let mut request_stream = request.into_inner();

        let basecaller_input_response_stream = async_stream::try_stream! {
            while let Some(dorado_request) = request_stream.next().await {
                let dorado_request = dorado_request?;

                basecaller_stdin.write_all(dorado_request.to_stdin().as_bytes()).await?;


                let response = BasecallerResponse { 
                    channel: 0, 
                    number: 0, 
                    id: "stdin-basecaller".to_string(),
                    stage: PipelineStage::BasecallerInput.into()
                };
                yield response
            }
        };
        
        let basecaller_output_classifier_input_response_stream = async_stream::try_stream! {
            while let Some(line) = basecaller_lines.next().await {
                let line = line?;
                
                // Repeating the basecaller test input/output for now
                classifier_stdin.write_all(line.as_bytes()).await?;  // return is stripped
                classifier_stdin.write_all("\n".as_bytes()).await?;


                let response = BasecallerResponse { 
                    channel: 0, 
                    number: 0, 
                    id: "stdout-basecaller".to_string(),
                    stage: PipelineStage::BasecallerClassifier.into()
                };

                yield response
            }
        };

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

                let response = BasecallerResponse { 
                    channel, 
                    number, 
                    id: "stdout-classifier".to_string(),
                    stage: PipelineStage::ClassifierOutput.into()
                };

                yield response
            }
        };

        // Merge output streams - this producees no guaranteed order on stage responses

        // For some reason `futures::stream::select_all` does not work - complains about mismatch
        // between the result async stream type, showing the same type?
        //
        // Might need to report this to async_stream crate, this looks a bit inefficient
        let response_stream_1 = futures::stream::select(
            basecaller_input_response_stream, 
            basecaller_output_classifier_input_response_stream,
        );
        let response_stream_2 = futures::stream::select(
            response_stream_1,
            classifier_output_response_stream,
        );

        Ok(Response::new(Box::pin(response_stream_2) as Self::BasecallDoradoStream))
    }

}

impl BasecallerRequest {
    pub fn to_stdin(&self) -> String {
        format!("{} {} {} {} {} {}\n", self.id, self.channel, self.number, 0, 0, join(&self.data, " "))
    }
}