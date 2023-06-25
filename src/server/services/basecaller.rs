
use crate::config::DoriConfig;
use crate::services::dori_api::basecaller::basecaller_server::Basecaller;
use crate::services::dori_api::basecaller::{BasecallerRequest, BasecallerResponse};

use std::pin::Pin;
// use std::io::{Read, BufRead, BufReader, Write};
use futures_core::Stream;
use futures_util::StreamExt;
use futures::stream::{select};
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


        let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<String>(1024);

        // Synchroneous basecaller process spawned as backgroudn task - is read within another spawned thread

        let mut basecaller_process = Command::new(self.config.dorado_path.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn basecaller process");

        let mut basecaller_stdin = basecaller_process.stdin.take().expect("Failed to open basecaller STDIN");

        let basecaller_stdout = basecaller_process.stdout.take().expect("Failed to open basecaller STDOUT");
        let mut lines = BufReader::new(basecaller_stdout).lines();


        let mut request_stream = request.into_inner();
        let response_stream = async_stream::try_stream! {
            while let Some(dorado_request) = request_stream.next().await {
                
                let dorado_request = dorado_request?;

                let read = BasecallerRead::from(dorado_request.clone()) ;

                basecaller_stdin.write_all(
                    format!("{}\n", serde_json::to_string(&read).unwrap()).as_bytes()
                ).await?;

                let response = BasecallerResponse { 
                    channel: read.channel, 
                    number: read.number, 
                    id: "from basecall stdin".to_string()
                };
                yield response
            }
        };


        // This works for some reason?? I have no idea what I am doing...
        // std::thread::spawn(move || {

        //     while let Some(line) = lines.next() {
        //         let line = line.unwrap();
        //         // log::info!("Received: {}", line);
        //         match response_tx.blocking_send(line) {
        //             Ok(_) => {},
        //             Err(_) => {}
        //         };
        //     }
        // });

        // std::thread::spawn(move || {

        //     while let Some(line) = response_rx.blocking_recv() {
        //         log::info!("Received something");
        //     }
        // });
        
        let dori_response_stream = async_stream::try_stream! {
            while let Some(line) = lines.next().await {
                let line = line.unwrap();
                let content: Vec<&str> = line.trim().split_whitespace().collect();
                log::info!("{:?}", content);
                let response = BasecallerResponse { 
                    channel: content[2].parse::<u32>().unwrap(), 
                    number: content[1].parse::<u32>().unwrap(), 
                    id: "streamoooooo".to_string()
                };
                yield response
            }
        };


        let response = select(response_stream, dori_response_stream);

        Ok(Response::new(Box::pin(response) as Self::BasecallDoradoStream))
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