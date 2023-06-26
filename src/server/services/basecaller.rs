
use crate::config::DoriConfig;
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

pub struct BasecallerService {
    config: DoriConfig
}
impl BasecallerService {
    pub fn new(config: &DoriConfig) -> Self {

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

        let mut basecaller_process = Command::new(self.config.dorado_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn basecaller process");

        let mut basecaller_stdin = basecaller_process.stdin.take().expect("Failed to open basecaller STDIN");
        let basecaller_stdout = basecaller_process.stdout.take().expect("Failed to open basecaller STDOUT");

        let mut basecaller_lines = BufReader::new(basecaller_stdout).lines();

        // Classification stage - at the moment just the same outputs with the minimal basecaller
        // input/output program to measure effects on latency

        let mut classifier_process = Command::new(self.config.dorado_path.as_os_str())
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

                let read = BasecallerRead::from(dorado_request.clone()) ;

                basecaller_stdin.write_all(
                    format!("{}\n", serde_json::to_string(&read).unwrap()).as_bytes()
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
        
        let basecaller_output_classifier_input_response_stream = async_stream::try_stream! {
            while let Some(line) = basecaller_lines.next().await {
                let line = line?;

                // Here we unpack the output of the C++ JSON parser which
                // currently stands-in for Dorado and for latency testing - 
                // important because we want the channel and read number in 
                // channel to send back a potential unblock request
                let content: Vec<&str> = line.trim().split_whitespace().collect();
                
                let number = content[1].parse::<u32>().unwrap();
                let channel = content[2].parse::<u32>().unwrap();

                // log::info!("{:?}", content);

                // Repeating the basecaller test input/output for now, without data
                // but with simulated long read identifier mimicking sequence reads of
                // ~ 180 bp * 2 (includes quality values) for ~ 800 signal values 
                let read = BasecallerRead {
                    number,
                    channel,
                    id: "A".repeat(360),  // this might introduce latency, let's see
                    data: Vec::new()
                };

                classifier_stdin.write_all(
                    format!("{}\n", serde_json::to_string(&read).unwrap()).as_bytes()
                ).await?;


                let response = BasecallerResponse { 
                    channel, 
                    number, 
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
                
                let number = content[1].parse::<u32>().unwrap();
                let channel = content[2].parse::<u32>().unwrap();

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

#[derive(serde::Serialize, Debug)]
struct BasecallerRead {
    id: String,
    data: Vec<u8>,
    channel: u32,
    number: u32
}
impl BasecallerRead {
    pub fn from(req: BasecallerRequest) -> Self {
        Self { id: req.id, data: req.data, channel: req.channel, number: req.number }
    }
}