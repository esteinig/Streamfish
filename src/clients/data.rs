use crate::services::minknow_api::data::get_channel_states_response::ChannelStateData;
use crate::services::minknow_api::data::get_channel_states_response::channel_state_data::State;
use crate::services::minknow_api::data::{GetChannelStatesRequest, GetChannelStatesResponse};
use crate::services::minknow_api::data::data_service_client::DataServiceClient;
use crate::clients::auth::AuthInterceptor;

use tonic::{Request, Streaming};
use tonic::transport::Error as TransportError;
use tonic::codegen::http::uri::InvalidUri;
use tonic::transport::Channel;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;

use crate::clients::minknow::MinknowClient;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DataClientError {
    /// Represents a failure to obtain the port of a position 
    /// likely due to that the position was not running at 
    /// initiation of the MinknowClient connection with the 
    /// ManagerService
    #[error("failed to obtain port of the requested position ({0})")]
    PortNotFound(String),
    // Represents failure to establish a channel due to invalid URI
    #[error("failed to configure a channel - invalid URI")]
    InvalidChannelUri(#[from] InvalidUri),
    // Represents failure to establish a channel due to invalid URI
    #[error("failed to configure a secure channel - invalid configuration of TLS")]
    InvalidTlsConfig(#[from] TransportError),
}


// A wrapper around the DataServiceClient, which requests 
// data and transforms responses for custom applications
pub struct DataClient {
    // A client instance with an active channel
    pub client: DataServiceClient<InterceptedService<Channel, AuthInterceptor>>
}
impl DataClient {
    // Create the data client from a newly opened channel 
    // or a clone of a channel - note that we always use 
    // an authenticated channel where each request is
    // intercepted to add the `local-auth` header
    pub fn new(channel: Channel, token: String) -> Self {

        let token: MetadataValue<Ascii> = token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = DataServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Self { client }
    }    
    pub async fn from_minknow_client(minknow_client: &MinknowClient, position_name: &str, watch_positions: bool) -> Result<Self, DataClientError> {

        // If we are not watching positions and their state to enable automated connection of this client to positions
        // that may not already be connected or running  (UNIMPLEMENTED) we try to get the secure RPC port of the named
        // position that is currently running

        let rpc_port = minknow_client.positions.get_secure_port(position_name).map_err(
            |_| DataClientError::PortNotFound(position_name.to_string())
        )?;

        let channel = Channel::from_shared(
            format!("https://{}:{}", minknow_client.config.host, rpc_port)
        ).map_err(
            |err| DataClientError::InvalidChannelUri(err)
        )?.tls_config(
            minknow_client.tls.clone()
        ).map_err(
            |err| DataClientError::InvalidTlsConfig(err)
        )?.connect().await?;

        let token: MetadataValue<Ascii> = minknow_client.config.token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = DataServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Ok(Self { client })
    }    
    // Get the current version information
    pub async fn stream_channel_state(
        &mut self, 
        first_channel: u32, 
        last_channel: u32, 
        use_channel_states_ids: Option<bool>, 
        wait_for_processing: bool, 
        heartbeat: Option<prost_types::Duration>
    ) -> Result<Streaming<GetChannelStatesResponse>, Box<dyn std::error::Error>>  {

        let request = Request::new(GetChannelStatesRequest {
            first_channel, last_channel, use_channel_states_ids, wait_for_processing, heartbeat
        });
        let stream = self.client.get_channel_states(request).await?.into_inner();

        Ok(stream)
    }
}



impl std::fmt::Display for ChannelStateData {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let channel_state_str = match &self.state {
            Some(state) => {
                match &state {
                    State::StateName(string) => string,
                    State::StateId(_) => "not implemented",
                    _ => "unknown"
                }
            },
            None => "unknown"
        };
        write!(f, "Channel {:0>4} => {} ", self.channel, channel_state_str)
    }
}