use crate::client::auth::AuthInterceptor;
use crate::services::minknow_api::device::{GetCalibrationRequest, GetCalibrationResponse, GetSampleRateRequest};
use crate::services::minknow_api::device::device_service_client::DeviceServiceClient;

use tonic::transport::Channel;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;

use crate::client::services::error::ClientError;
use crate::client::minknow::MinKnowClient;

#[derive(Debug, Clone)]
pub struct DeviceCalibration {
    pub digitisation: u32,
    pub offsets: Vec<f32>,
    pub pa_ranges: Vec<f32>,
    pub has_calibraton: bool
}
impl DeviceCalibration {
    pub fn from(response: GetCalibrationResponse) -> Self {
        Self {
            digitisation: response.digitisation,
            offsets: response.offsets,
            pa_ranges: response.pa_ranges,
            has_calibraton: response.has_calibration
        }
    }
}

// A wrapper around the DeviceServiceClient, which requests 
// data and transforms responses for custom applications
pub struct DeviceClient {
    // A client instance with an active channel
    pub client: DeviceServiceClient<InterceptedService<Channel, AuthInterceptor>>
}
impl DeviceClient {
    pub fn new(channel: Channel, token: String) -> Self {

        let token: MetadataValue<Ascii> = token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = DeviceServiceClient::with_interceptor(channel, AuthInterceptor { token });

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
        let client = DeviceServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Ok(Self { client })
    }   
    // This may be important for the basecaller to access digitisation, offsets and scale ADC values to pA 
    pub async fn get_calibration(&mut self, first_channel: &u32, last_channel: &u32) -> Result<DeviceCalibration, Box<dyn std::error::Error>> {

        let response = self.client.get_calibration(
            tonic::Request::new(GetCalibrationRequest { first_channel: first_channel.clone(), last_channel: last_channel.clone() })
        ).await?.into_inner();

        Ok(DeviceCalibration::from(response))
    }
    // This may be important for the basecaller to access - see proto file documentation for explanation
    pub async fn get_sample_rate(&mut self) -> Result<u32, Box<dyn std::error::Error>> {

        let response = self.client.get_sample_rate(
            tonic::Request::new(GetSampleRateRequest { })
        ).await?.into_inner();

        Ok(response.sample_rate)

    }

}