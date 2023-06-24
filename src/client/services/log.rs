use crate::services::minknow_api::log::{UserMessage, SendUserMessageRequest, SendUserMessageResponse};
use crate::services::minknow_api::log::log_service_client::LogServiceClient;
use crate::client::auth::AuthInterceptor;

use tonic::transport::Channel;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;

use crate::client::services::error::ClientError;
use crate::client::minknow::MinKnowClient;



// A wrapper around the AnalysisConfigurationServiceClient, which requests 
// data and transforms responses for custom applications
pub struct LogClient {
    // A client instance with an active channel
    pub client: LogServiceClient<InterceptedService<Channel, AuthInterceptor>>
}
impl LogClient {
    pub fn new(channel: Channel, token: String) -> Self {

        let token: MetadataValue<Ascii> = token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = LogServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Self { client }
    }    
    pub async fn from_minknow_client(minknow_client: &MinKnowClient, position_name: &str) -> Result<Self, ClientError> {

        let rpc_port = minknow_client.positions.get_secure_port(position_name).map_err(
            |_| ClientError::PortNotFound(position_name.to_string())
        )?;

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
        let client = LogServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Ok(Self { client })
    }   
    
}