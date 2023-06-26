//! Dori is a basecall and analysis RPC implementation for Reefsquid::ReadUntilClient 

use tower::service_fn;
use tokio::net::{UnixListener, UnixStream};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Server, Endpoint, Channel};

use crate::config::DoriConfig;
use crate::server::services::basecaller::BasecallerService;
use crate::services::dori_api::basecaller::BasecallerRequest;
use crate::services::dori_api::basecaller::basecaller_server::BasecallerServer;
use crate::services::dori_api::basecaller::basecaller_client::BasecallerClient;

pub struct DoriServer { }

impl DoriServer {
    pub async fn run(config: &DoriConfig) -> Result<(), Box<dyn std::error::Error>> {

        let basecaller_service = BasecallerService::new(config);

        let uds_path_parent_dir = config.uds_path.parent().unwrap();

        if config.uds_path.exists() && config.uds_path_override {
            std::fs::remove_file(&config.uds_path)?;
            log::warn!("UDS override configured! Replaced existing socket: {}", config.uds_path.display());
        }

        if !uds_path_parent_dir.exists() {
            std::fs::create_dir_all(config.uds_path.parent().unwrap())?;
            log::debug!("UDS parent directory created at: {}", &config.uds_path.display());
        }

        let uds = UnixListener::bind(&config.uds_path)?;
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
    pub async fn connect(config: &DoriConfig) -> Result<Self, Box<dyn std::error::Error>> {
    
        let uds_path = config.uds_path.clone();
            
        // We will ignore this URI because UDS do not use it
        let channel = Endpoint::try_from("http://[::]:50051")?
            .connect_with_connector(service_fn(move |_|  { 
                UnixStream::connect(uds_path.clone()) 
        })).await?;

        Ok(Self { client: BasecallerClient::new(channel) })
    }
    // Dorado basecall request/response stream
    pub async fn test_basecall_dorado(&mut self) -> Result<(), Box<dyn std::error::Error>> {

        let dorado_request_stream = async_stream::stream! {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(1)
            );
            loop {
                let _ = interval.tick().await;
                let request = BasecallerRequest { id: String::from("TestID"), data: vec![0], number: 0, channel: 0 };
                log::info!("Sending request to Dori::BasecallDorado");
                yield request
            }
        };

        let mut dorado_response_stream = self.client.basecall_dorado(
            tonic::Request::new(dorado_request_stream)
        ).await?.into_inner();

        while let Some(_) = dorado_response_stream.message().await? {
           log::info!("Received response from Dori::BasecallDorado")

        }

        Ok(())
    }
}