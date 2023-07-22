use tonic::transport::Channel;
use crate::client::error::ClientError;
use crate::client::auth::AuthInterceptor;
use crate::client::minknow::MinknowClient;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;
use crate::services::minknow_api::analysis_configuration::analysis_configuration_service_client::AnalysisConfigurationServiceClient;


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
    pub async fn from_minknow_client(minknow_client: &MinknowClient, position_name: &str) -> Result<Self, ClientError> {

        
        let rpc_port = match minknow_client.icarust.enabled {
            true => minknow_client.icarust.position_port, 
            false =>  minknow_client.positions.get_secure_port(position_name).map_err(
            |_| ClientError::PortNotFound(position_name.to_string())
            )?
        };

      
        let channel = Channel::from_shared(
            format!("https://{}:{}", minknow_client.config.host, rpc_port)
        ).map_err(|_| ClientError::InvalidUri)?
         .tls_config(minknow_client.tls.clone())
         .map_err( |_| ClientError::InvalidTls)?
         .connect().await
         .map_err(|_| ClientError::ControlServerConnectionInitiation)?;

        let token: MetadataValue<Ascii> = minknow_client.config.token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = AnalysisConfigurationServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Ok(Self { client })
    }  
}