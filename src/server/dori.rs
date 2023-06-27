//! Dori is a basecall and analysis RPC implementation for Streamfish::ReadUntilClient 

use tower::service_fn;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Server, Endpoint, Channel};

use crate::config::StreamfishConfig;
use crate::server::services::basecaller::BasecallerService;
use crate::services::dori_api::basecaller::basecaller_server::BasecallerServer;
use crate::services::dori_api::basecaller::basecaller_client::BasecallerClient;

pub struct DoriServer { }

impl DoriServer {
    pub async fn run(config: StreamfishConfig) -> Result<(), Box<dyn std::error::Error>> {

        let basecaller_service = BasecallerService::new(&config);

        let uds_path_parent_dir = &config.dori.uds_path.parent().unwrap();

        if config.dori.uds_path.exists() && config.dori.uds_path_override {
            std::fs::remove_file(&config.dori.uds_path)?;
            log::warn!("UDS override configured! Replaced existing socket: {}", &config.dori.uds_path.display());
        }

        if !uds_path_parent_dir.exists() {
            std::fs::create_dir_all(&config.dori.uds_path.parent().unwrap())?;
            log::debug!("UDS parent directory created at: {}", &config.dori.uds_path.display());
        }

        let uds = UnixListener::bind(&config.dori.uds_path)?;
        let uds_stream = UnixListenerStream::new(uds);

        Server::builder()
            .add_service(BasecallerServer::new(basecaller_service))
            .serve_with_incoming(uds_stream)
            .await?;

        Ok(())
    }
}

pub struct DoriClient { 
    pub client: BasecallerClient<Channel>
}

impl DoriClient {
    pub async fn connect(config: &StreamfishConfig) -> Result<Self, Box<dyn std::error::Error>> {
    
        let uds_path = config.dori.uds_path.clone();
            
        // We will ignore this URI because UDS do not use it
        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector(service_fn(move |_|  { 
                UnixStream::connect(uds_path.clone()) 
        })).await?;

        Ok(Self { client: BasecallerClient::new(channel) })
    }
}