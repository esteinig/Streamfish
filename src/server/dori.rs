//! Dori is a basecall and analysis RPC implementation for Streamfish::ReadUntilClient 

use tokio::net::UnixListener;
use tonic::transport::Server;
use crate::config::{StreamfishConfig, ServerType};
use crate::server::error::ServerError;
use tokio_stream::wrappers::UnixListenerStream;
use crate::server::services::adaptive::AdaptiveSamplingService;
use crate::server::services::dynamic::DynamicFeedbackService;
use crate::services::dori_api::adaptive::adaptive_sampling_server::AdaptiveSamplingServer;
use crate::services::dori_api::dynamic::dynamic_feedback_server::DynamicFeedbackServer;

pub struct DoriServer { }

// Very explicit but easiest to duplicate the server type configs for now
impl DoriServer {
    pub async fn run(config: StreamfishConfig, server_type: ServerType) -> Result<(), ServerError> {

        match server_type {
            ServerType::Adaptive => {

                let service = AdaptiveSamplingService::new(&config);
                let dori_config = config.dori.adaptive;

                if dori_config.tcp_enabled {

                    let address = format!("{}:{}", dori_config.tcp_host, dori_config.tcp_port).parse().map_err(|_| ServerError::InvalidSocketAddress)?;

                    log::info!("Dori AdaptiveSamplingTCP connection listening on: {}", address);

                    Server::builder()
                        .add_service(AdaptiveSamplingServer::new(service))
                        .serve(address)
                        .await.map_err(|_| ServerError::ServeTcp)?;
                    
                } else {
                    let uds_path_parent_dir = match dori_config.uds_path.parent() {
                        Some(path) => path,
                        None => return Err(ServerError::UnixDomainSocketParentDirectoryPath)
                    };

                    if dori_config.uds_path.exists() && dori_config.uds_override {
                        std::fs::remove_file(&dori_config.uds_path).map_err(|_| ServerError::UnixDomainSocketRemove)?;
                        log::warn!("Replaced existing socket: {}", dori_config.uds_path.display());
                    }
            
                    if !uds_path_parent_dir.exists() {
                        std::fs::create_dir_all(uds_path_parent_dir).map_err(|_| ServerError::UnixDomainSocketParentDirectoryCreate)?;
                        log::debug!("Dori AdaptiveSampling UDS parent directory created at: {}", &dori_config.uds_path.display());
                    }
            
                    let uds = UnixListener::bind(&dori_config.uds_path).map_err(|_| ServerError::UnixDomainSocketListener)?;
                    let uds_stream = UnixListenerStream::new(uds);
            
                    Server::builder()
                        .add_service(AdaptiveSamplingServer::new(service))
                        .serve_with_incoming(uds_stream)
                        .await.map_err(|_| ServerError::ServeUds)?;
                }

            },
            ServerType::Dynamic =>   {

                let service = DynamicFeedbackService::new(&config);
                let dori_config = config.dori.dynamic;

                if dori_config.tcp_enabled {

                    let address = format!("{}:{}", dori_config.tcp_host, dori_config.tcp_port).parse().map_err(|_| ServerError::InvalidSocketAddress)?;

                    log::info!("Dori DynamicFeedback TCP connection listening on: {}", address);

                    Server::builder()
                        .add_service(DynamicFeedbackServer::new(service))
                        .serve(address)
                        .await.map_err(|_| ServerError::ServeTcp)?;
                    
                } else {
                    let uds_path_parent_dir = match dori_config.uds_path.parent() {
                        Some(path) => path,
                        None => return Err(ServerError::UnixDomainSocketParentDirectoryPath)
                    };

                    if dori_config.uds_path.exists() && dori_config.uds_override {
                        std::fs::remove_file(&dori_config.uds_path).map_err(|_| ServerError::UnixDomainSocketRemove)?;
                        log::warn!("Replaced existing socket: {}", dori_config.uds_path.display());
                    }
            
                    if !uds_path_parent_dir.exists() {
                        std::fs::create_dir_all(uds_path_parent_dir).map_err(|_| ServerError::UnixDomainSocketParentDirectoryCreate)?;
                        log::debug!("Dori DynamicFeedback  UDS parent directory created at: {}", &dori_config.uds_path.display());
                    }
            
                    let uds = UnixListener::bind(&dori_config.uds_path).map_err(|_| ServerError::UnixDomainSocketListener)?;
                    let uds_stream = UnixListenerStream::new(uds);
            
                    Server::builder()
                        .add_service(DynamicFeedbackServer::new(service))
                        .serve_with_incoming(uds_stream)
                        .await.map_err(|_| ServerError::ServeUds)?;
                }

            }     
        }
        
        Ok(())
    }
}
