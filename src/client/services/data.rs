
use tonic::transport::Channel;
use tonic::{Request, Streaming};
use crate::client::error::ClientError;
use crate::client::auth::AuthInterceptor;
use crate::client::minknow::MinknowClient;
use tonic::metadata::{MetadataValue, Ascii};
use tonic::service::interceptor::InterceptedService;
use crate::services::minknow_api::data::data_service_client::DataServiceClient;
use crate::services::minknow_api::data::get_channel_states_response::ChannelStateData;
use crate::services::minknow_api::data::{GetChannelStatesRequest, GetChannelStatesResponse};
use crate::services::minknow_api::data::get_channel_states_response::channel_state_data::State;



use colored::*;


// A wrapper around the DataServiceClient, which requests 
// data and transforms responses for custom applications
pub struct DataClient {
    // A client instance with an active channel
    pub client: DataServiceClient<InterceptedService<Channel, AuthInterceptor>>
}
impl DataClient {
    pub fn new(channel: Channel, token: String) -> Self {

        let token: MetadataValue<Ascii> = token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = DataServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Self { client }
    }    
    pub async fn from_minknow_client(minknow_client: &MinknowClient, position_name: &str) -> Result<Self, ClientError> {

        let rpc_port = match &minknow_client.icarust_config {
            Some(icarust_config) => {
                match icarust_config.enabled {
                    true => icarust_config.position_port, 
                    false =>  minknow_client.positions.get_secure_port(position_name).map_err(|_| ClientError::PortNotFound(position_name.to_string()))?
                 }
            },
            None => minknow_client.positions.get_secure_port(position_name).map_err(|_| ClientError::PortNotFound(position_name.to_string()))?
        };

        let channel = Channel::from_shared(
            format!("https://{}:{}", minknow_client.config.host, rpc_port)
        ).map_err(|_| ClientError::InvalidUri)?
         .tls_config(minknow_client.tls.clone())
         .map_err( |_| ClientError::InvalidTls)?
         .connect().await
         .map_err(|_| ClientError::ControlServerConnectionInitiation)?;

        let token: MetadataValue<Ascii> = minknow_client.config.token.parse().expect("Failed to parse token into correct format (ASCII)");
        let client = DataServiceClient::with_interceptor(channel, AuthInterceptor { token });

        Ok(Self { client })
    }    
    // Stream the channel states
    pub async fn stream_channel_states(
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
                    State::StateName(string) => match string.as_str() {
                        "strand" => string.bright_green(),
                        "unavailable" => string.blue().dimmed(),
                        "pore" => string.green(),
                        "unblocking" => string.bright_magenta(),
                        "pending_mux_change" => string.magenta().dimmed(),
                        _ => string.black().dimmed()
                    },
                    State::StateId(_) => "not implemented".white()
                }
            },
            None => "unknown".white()
        };
        write!(f, "Channel {:>4} => {} ", self.channel, channel_state_str)
    }
}