use tonic::{Request, Response, Status};

use std::pin::Pin;
use futures_core::Stream;
use futures_util::StreamExt;

use crate::config::StreamfishConfig;
use crate::services::dori_api::dynamic::dynamic_feedback_server::DynamicFeedback;
use crate::services::dori_api::dynamic::{DynamicFeedbackRequest, DynamicFeedbackResponse, DynamicTarget, RequestType};


pub struct DynamicFeedbackService {
    config: StreamfishConfig
}
impl DynamicFeedbackService {

    pub fn new(config: &StreamfishConfig) -> Self {
        Self { config: config.clone() }
    }
}

#[tonic::async_trait]
impl DynamicFeedback for DynamicFeedbackService {

    type TestStream = Pin<Box<dyn Stream<Item = Result<DynamicFeedbackResponse, Status>> + Send  + 'static>>;

    async fn test(&self, request: Request<tonic::Streaming<DynamicFeedbackRequest>>) -> Result<Response<Self::TestStream>, Status> {
        
        let init_dynamic_request: i32 = RequestType::Init.into();

        let (response_tx, mut response_rx) = tokio::sync::mpsc::channel(1024); // drains into output stream

        let mut request_stream = request.into_inner();

        /*  Dynamic workflow controller service for pore target assignment HashMap<channel_number, ExperimentConfig>
    
            ExperimentConfig is also used to determine the number of unique required threads with minimap2-rs to which the basecalled sequences are routed

         */

        let config = self.config.clone();

        tokio::spawn(async move {        
            log::info!("Starting dynamic request stream handler");
            while let Some(dynamic_request) = request_stream.next().await {
                
                let dynamic_request = match dynamic_request {
                    Ok(request) => request,
                    Err(_) => {
                        log::warn!("Failed to parse request from incoming stream, continue stream...");
                        continue
                    }
                };

                if dynamic_request.request == init_dynamic_request {
                    log::info!("Received initiation request on dynamic test endpoint");
                    // No action, continue and wait for data stream
                    continue;
                }

                let test_targets: Vec<DynamicTarget> = config.dynamic.test_targets.iter().map(|t| t.to_dynamic_target()).collect();

                response_tx.send(DynamicFeedbackResponse {
                    targets: test_targets.into_iter().filter(|t| !dynamic_request.targets.contains(t)).collect()
                }).await.expect("Failed to send response into output stream");

            }
        });

        let pipeline_response_stream = async_stream::try_stream! {
            while let Some(response) = response_rx.recv().await {
                yield response
            }
        };

        Ok(Response::new(Box::pin(pipeline_response_stream) as Self::TestStream))
    }



}