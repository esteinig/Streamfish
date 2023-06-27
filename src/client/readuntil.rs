use::colored::*;

use crate::config::{
    StreamfishConfig, 
    ReadUntilConfig
};

use crate::server::dori::DoriClient;
use crate::client::minknow::MinKnowClient;

use crate::services::dori_api::basecaller::{
    BasecallerRequest,
    BasecallerResponse
};

pub struct ReadUntilClient {
    pub dori: DoriClient,
    pub minknow: MinKnowClient,
    pub readuntil: ReadUntilConfig,
}

// Do not use Strings when it can be avoided, introduces too much latency, use static strings (&str) or enumerations
// this introducted a bit of latency into the logging as string name conversion

impl ReadUntilClient {

    pub async fn connect(config: &StreamfishConfig) -> Result<Self, Box<dyn std::error::Error>> {

        Ok(Self { 
            dori: DoriClient::connect(&config).await?, 
            minknow: MinKnowClient::connect(&config.minknow).await?,
            readuntil: config.readuntil.clone(),
        })
    }

    pub async fn run(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {


        log::info!("Request to Dori to start adaptive sampling");

        // BasecallService response stream is initiated with the dori request stream
        let dori_request = tonic::Request::new(BasecallerRequest {  });
        let mut dori_stream = self.dori.client.basecall_dorado(dori_request).await?.into_inner();
        
        // ========================================
        // DoriService response stream is processed
        // ========================================

        while let Some(dori_response) = dori_stream.message().await.expect("Failed to get response from Dori response stream") {

            log::info!("Channel {:<5} => {}", &dori_response.channel, &dori_response.id);

        }

        Ok(())

    }
}




