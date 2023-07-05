use crate::services::minknow_api::acquisition::{CurrentStatusRequest, CurrentStatusResponse};
use crate::services::minknow_api::acquisition::acquisition_service_client::AcquisitionServiceClient;
use crate::client::auth::AuthInterceptor;

use tonic::transport::Channel;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;

use crate::client::services::error::ClientError;
use crate::client::minknow::MinKnowClient;



// A wrapper around the AcquisitionServiceClient, which requests 
// data and transforms responses for custom applications
pub struct AcquisitionClient {
    // A client instance with an active channel
    pub client: AcquisitionServiceClient<InterceptedService<Channel, AuthInterceptor>>
}
impl AcquisitionClient {
    pub fn new(channel: Channel, token: String) -> Self {

        let token: MetadataValue<Ascii> = token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = AcquisitionServiceClient::with_interceptor(channel, AuthInterceptor { token });

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
        let client = AcquisitionServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Ok(Self { client })
    }   
    pub async fn get_current_status(&mut self) -> Result<CurrentStatusResponse, Box<dyn std::error::Error>> {

        let request = tonic::Request::new(CurrentStatusRequest {});
        let response = self.client.current_status(request).await?.into_inner();
        Ok(response)
    }
}