use tower::service_fn;
use tokio::net::UnixStream;
use crate::client::error::ClientError;
use tonic::transport::{Endpoint, Channel};
use crate::{services::dori_api::adaptive::adaptive_sampling_client::AdaptiveSamplingClient, config::StreamfishConfig};

#[derive(Debug, Clone)]
pub struct DoriClient { 
    pub client: AdaptiveSamplingClient<Channel>
}

impl DoriClient {
    pub async fn connect(config: &StreamfishConfig) -> Result<Self, ClientError> {
        
        if config.dori.tcp_enabled {
            
            let address = format!("http://{}:{}", config.readuntil.dori_tcp_host, config.readuntil.dori_tcp_port);

            log::info!("Dori client connecting to: {}", &address);

            let channel = Channel::from_shared(address.clone()).map_err(|_| ClientError::InvalidUri)?.connect().await.map_err(|_| ClientError::DoriServerConnectionInitiation)?;

            log::info!("Dori client connected on TCP channel");

            Ok(Self {  client: AdaptiveSamplingClient::new(channel) })

        } else {
            let uds_path = config.dori.uds_path.clone();

            log::info!("Dori client connecting to: {}", &uds_path.display());
            
            // We will ignore this URI because UDS do not use it
            let channel = Endpoint::try_from("http://[::]:50051").map_err(|_| ClientError::DoriServerConnectionInitiation)?
                .connect_with_connector(service_fn(move |_|  { 
                    UnixStream::connect(uds_path.clone()) 
            })).await.map_err(|_| ClientError::DoriServerConnectionInitiation)?;

            log::info!("Dori client connected on UDS channel");

            Ok(Self { client: AdaptiveSamplingClient::new(channel) })
        }
    }
}