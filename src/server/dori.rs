//! Dori is a basecall and analysis RPC implementation for Streamfish::ReadUntilClient 

use tower::service_fn;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Server, Endpoint, Channel};

use crate::config::StreamfishConfig;
use crate::server::services::adaptive::AdaptiveSamplingService;
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSamplingServer;
use crate::services::dori_api::adaptive::adaptive_sampling_client::AdaptiveSamplingClient;

pub struct DoriServer { }

impl DoriServer {
    pub async fn run(config: &StreamfishConfig) -> Result<(), Box<dyn std::error::Error>> {

        let service = AdaptiveSamplingService::new(config);

        let uds_path_parent_dir = config.dori.uds_path.parent().unwrap();

        if config.dori.uds_path.exists() && config.dori.uds_path_override {
            std::fs::remove_file(&config.dori.uds_path)?;
            log::warn!("UDS override configured! Replaced existing socket: {}", config.dori.uds_path.display());
        }

        if !uds_path_parent_dir.exists() {
            std::fs::create_dir_all(config.dori.uds_path.parent().unwrap())?;
            log::debug!("UDS parent directory created at: {}", &config.dori.uds_path.display());
        }

        let uds = UnixListener::bind(&config.dori.uds_path)?;
        let uds_stream = UnixListenerStream::new(uds);

        Server::builder()
            .add_service(AdaptiveSamplingServer::new(service))
            .serve_with_incoming(uds_stream)
            .await?;

        Ok(())
    }
}

pub struct DoriClient { 
    pub client: AdaptiveSamplingClient<Channel>
}

impl DoriClient {
    pub async fn connect(config: &StreamfishConfig) -> Result<Self, Box<dyn std::error::Error>> {
    
        let uds_path = config.dori.uds_path.clone();
            
        // We will ignore this URI because UDS do not use it
        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector(service_fn(move |_|  { 
                UnixStream::connect(uds_path.clone()) 
        })).await?;

        Ok(Self { client: AdaptiveSamplingClient::new(channel) })
    }
}