use tower::service_fn;
use tokio::net::UnixStream;
use crate::{client::error::ClientError};
use tonic::transport::{Endpoint, Channel};
use crate::{services::dori_api::adaptive::adaptive_sampling_client::AdaptiveSamplingClient as AdaptiveSamplingClientRpc, config::StreamfishConfig};
use crate::services::dori_api::dynamic::dynamic_feedback_client::DynamicFeedbackClient as DynamicFeedbackclientRpc;

#[derive(Debug, Clone)]
pub struct AdaptiveSamplingClient { 
    pub client: AdaptiveSamplingClientRpc<Channel>
}

impl AdaptiveSamplingClient {
    pub async fn connect(config: &StreamfishConfig) -> Result<Self, ClientError> {
        if config.dori.adaptive.tcp_enabled {
            
            let address = format!("http://{}:{}", config.readuntil.dori_tcp_host, config.readuntil.dori_tcp_port);

            log::info!("Dori AdaptiveSampling client connecting to: {}", &address);

            let channel = Channel::from_shared(address.clone()).map_err(|_| ClientError::InvalidUri)?.connect().await.map_err(|_| ClientError::DoriServerConnectionInitiation)?;

            log::info!("Dori AdaptiveSampling client connected on TCP channel");

            Ok(Self { client: AdaptiveSamplingClientRpc::new(channel) })

        } else {
            let uds_path = config.dori.adaptive.uds_path.clone();

            log::info!("Dori AdaptiveSampling client connecting to: {}", &uds_path.display());
            
            // We will ignore this URI because UDS do not use it
            let channel = Endpoint::try_from("http://[::]:50051").map_err(|_| ClientError::DoriServerConnectionInitiation)?
                .connect_with_connector(service_fn(move |_|  { 
                    UnixStream::connect(uds_path.clone()) 
            })).await.map_err(|_| ClientError::DoriServerConnectionInitiation)?;

            log::info!("Dori AdaptiveSampling client connected on UDS channel");

            Ok(Self { client: AdaptiveSamplingClientRpc::new(channel) })
        }
    }
}

#[derive(Debug, Clone)]
pub struct DynamicFeedbackClient { 
    pub client: DynamicFeedbackclientRpc<Channel>
}

impl DynamicFeedbackClient {
    pub async fn connect(config: &StreamfishConfig) -> Result<Self, ClientError> {
        if config.dori.dynamic.tcp_enabled {
            
            let address = format!("http://{}:{}", config.readuntil.dori_tcp_host, config.readuntil.dori_tcp_port);

            log::info!("Dori DynamicFeedback client connecting to: {}", &address);

            let channel = Channel::from_shared(address.clone()).map_err(|_| ClientError::InvalidUri)?.connect().await.map_err(|_| ClientError::DoriServerConnectionInitiation)?;

            log::info!("Dori AdaptiveSampling client connected on TCP channel");

            Ok(Self { client: DynamicFeedbackclientRpc::new(channel) })

        } else {
            let uds_path = config.dori.dynamic.uds_path.clone();

            log::info!("Dori AdaptiveSampling client connecting to: {}", &uds_path.display());
            
            // We will ignore this URI because UDS do not use it
            let channel = Endpoint::try_from("http://[::]:50051").map_err(|_| ClientError::DoriServerConnectionInitiation)?
                .connect_with_connector(service_fn(move |_|  { 
                    UnixStream::connect(uds_path.clone()) 
            })).await.map_err(|_| ClientError::DoriServerConnectionInitiation)?;

            log::info!("Dori AdaptiveSampling client connected on UDS channel");

            Ok(Self { client: DynamicFeedbackclientRpc::new(channel) })
        }
    }
}