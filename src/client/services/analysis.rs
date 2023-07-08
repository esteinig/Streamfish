use std::collections::HashMap;
use std::collections::hash_map::RandomState;

use crate::services::minknow_api::analysis_configuration::{GetReadClassificationsRequest, GetAnalysisConfigurationRequest};
use crate::services::minknow_api::analysis_configuration::analysis_configuration_service_client::AnalysisConfigurationServiceClient;
use crate::client::auth::AuthInterceptor;

use tonic::transport::Channel;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;

use crate::client::services::error::{ClientError, AnalysisConfigurationClientError};
use crate::client::minknow::MinKnowClient;



// A wrapper around the AnalysisConfigurationServiceClient, which requests 
// data and transforms responses for custom applications
pub struct AnalysisConfigurationClient {
    // A client instance with an active channel
    pub client: AnalysisConfigurationServiceClient<InterceptedService<Channel, AuthInterceptor>>
}
impl AnalysisConfigurationClient {
    pub fn new(channel: Channel, token: String) -> Self {

        let token: MetadataValue<Ascii> = token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = AnalysisConfigurationServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Self { client }
    }    
    pub async fn from_minknow_client(minknow_client: &MinKnowClient, position_name: &str) -> Result<Self, ClientError> {

        
        let rpc_port = match minknow_client.icarust.enabled {
            true => minknow_client.icarust.position_port, 
            false =>  minknow_client.positions.get_secure_port(position_name).map_err(
            |_| ClientError::PortNotFound(position_name.to_string())
            )?
        };

        let channel = Channel::from_shared(
            format!("https://{}:{}", minknow_client.config.host, rpc_port)
        ).map_err(
            |err| ClientError::InvalidChannelUri(err)
        )?.tls_config(
            minknow_client.tls.clone()
        ).map_err(
            |err| ClientError::InvalidTlsConfig(err)
        )?.connect().await?;
        let token: MetadataValue<Ascii> = minknow_client.config.token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = AnalysisConfigurationServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Ok(Self { client })
    }   
    // Obtain the read classifications from the RPC endpoint for the current analysis configuration - could be important to set correct strand values for ReadUntilClient 
    pub async fn get_read_classifications(&mut self) -> Result<HashMap<i32, String, RandomState>, Box<dyn std::error::Error>> {

        let response = self.client.get_read_classifications(
            tonic::Request::new(GetReadClassificationsRequest {})
        ).await?.into_inner();

        Ok(response.read_classifications)

    }
    // Obtain the analysis configuration and set specific values - important for latency in  ReadUntilClient 
    //
    // Currently DOES NOT WORK - is overwritten by changes in the sequencing TOML file: https://github.com/nanoporetech/minknow_api/issues/18
    pub async fn set_analysis_configuration(&mut self, break_reads_after_seconds: f64) -> Result<(), Box<dyn std::error::Error>> {

        // Get
        let mut analysis_config = self.client.get_analysis_configuration(
            tonic::Request::new(GetAnalysisConfigurationRequest {})
        ).await?.into_inner();

        // Mod
        let read_detection_update = match analysis_config.read_detection {
            Some(mut read_detection_params) => {
                read_detection_params.break_reads_after_seconds = Some(break_reads_after_seconds);
                read_detection_params
            },
            None => return Err(Box::new(AnalysisConfigurationClientError::ReadDetectionParamsNotFound))
        };
        analysis_config.read_detection = Some(read_detection_update);

        // Set
        self.client.set_analysis_configuration(tonic::Request::new(analysis_config)).await?;

        log::info!("NOT FUNCTIONAL - modified analysis configuration: read_detection.break_reads_after_seconds = {}", break_reads_after_seconds);

        Ok(())

    }
}