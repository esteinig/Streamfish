
use crate::services::dori_api::basecaller::basecaller_server::Basecaller;
use crate::services::dori_api::basecaller::{DoradoBasecallerRequest, DoradoBasecallerResponse};


use std::pin::Pin;
use futures_core::Stream;
use futures_util::StreamExt;
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct BasecallerService;

// Service implementation matches the RPC definitions translated during build - see the trait annotations on BasecallerService
#[tonic::async_trait]
impl Basecaller for BasecallerService {

    type BasecallDoradoStream = Pin<Box<dyn Stream<Item = Result<DoradoBasecallerResponse, Status>> + Send  + 'static>>;

    async fn basecall_dorado( 
        &self,
        request: Request<tonic::Streaming<DoradoBasecallerRequest>>,
    ) -> Result<Response<Self::BasecallDoradoStream>, Status> {
        
        let mut stream = request.into_inner();
        let output = async_stream::try_stream! {
            while let Some(dorado_request) = stream.next().await {
                
                let dorado_request = dorado_request?;  // make sure to catch errors

                log::info!("Channel {:>5} ({}) => {}", dorado_request.channel, dorado_request.number, dorado_request.id);  // server log

                yield  DoradoBasecallerResponse { channel: dorado_request.channel, number: dorado_request.number, id: dorado_request.id }
            }
        };
        Ok(Response::new(Box::pin(output) as Self::BasecallDoradoStream))
    }

}

