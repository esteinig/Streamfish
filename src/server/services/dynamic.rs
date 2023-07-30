use tonic::{Request, Response, Status};

use crate::config::StreamfishConfig;
use crate::services::dori_api::dynamic::dynamic_feedback_server::DynamicFeedback;
use crate::services::dori_api::dynamic::{TestRequest, TestResponse, DynamicTarget};


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

    async fn test(&self, request: Request<TestRequest>) -> Result<Response<TestResponse>, Status> {
        
        let test_targets: Vec<DynamicTarget> = self.config.dynamic.test_targets.iter().map(|t| t.to_dynamic_target()).collect();
        let test_request = request.into_inner();

        let response = TestResponse {
            targets: test_targets.into_iter().filter(|t| !test_request.targets.contains(t)).collect()
        };

        Ok(Response::new(response))
    }
}