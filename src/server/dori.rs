//! Dori is a basecall and analysis RPC implementation for Streamfish::ReadUntilClient 

use tokio::net::UnixListener;
use tonic::transport::Server;
use crate::config::StreamfishConfig;
use crate::server::error::ServerError;
use tokio_stream::wrappers::UnixListenerStream;
use crate::server::services::adaptive::AdaptiveSamplingService;
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSamplingServer;

pub struct DoriServer { }

impl DoriServer {
    pub async fn run(config: StreamfishConfig) -> Result<(), ServerError> {

        let service = AdaptiveSamplingService::new(&config);

        if config.dori.tcp_enabled {

            let address = format!("{}:{}", config.dori.tcp_host, config.dori.tcp_port).parse().map_err(|_| ServerError::InvalidSocketAddress)?;

            log::info!("Dori TCP connection listening on: {}", address);

            Server::builder()
                .add_service(AdaptiveSamplingServer::new(service))
                .serve(address)
                .await.map_err(|_| ServerError::ServeTcp)?;
            
        } else {
            let uds_path_parent_dir = match config.dori.uds_path.parent() {
                Some(path) => path,
                None => return Err(ServerError::UnixDomainSocketParentDirectoryPath)
            };

            if config.dori.uds_path.exists() && config.dori.uds_override {
                std::fs::remove_file(&config.dori.uds_path).map_err(|_| ServerError::UnixDomainSocketRemove)?;
                log::warn!("UDS override configured! Replaced existing socket: {}", config.dori.uds_path.display());
            }
    
            if !uds_path_parent_dir.exists() {
                std::fs::create_dir_all(uds_path_parent_dir).map_err(|_| ServerError::UnixDomainSocketParentDirectoryCreate)?;
                log::debug!("UDS parent directory created at: {}", &config.dori.uds_path.display());
            }
    
            let uds = UnixListener::bind(&config.dori.uds_path).map_err(|_| ServerError::UnixDomainSocketListener)?;
            let uds_stream = UnixListenerStream::new(uds);
    
            Server::builder()
                .add_service(AdaptiveSamplingServer::new(service))
                .serve_with_incoming(uds_stream)
                .await.map_err(|_| ServerError::ServeUds)?;
        }
        
        Ok(())
    }
}
