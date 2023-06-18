#![allow(dead_code)]

use clap::Parser;

use tonic::transport::{Channel, Certificate};
use crate::terminal::{App, Commands};
use crate::config::ReefSquidConfig;
use crate::utils::init_logger;
use tonic::transport::ClientTlsConfig;

use crate::services::minknow_api::manager::manager_service_client::ManagerServiceClient;
use crate::services::minknow_api::manager::GetVersionInfoRequest;

mod services;
mod terminal;
mod config;
mod utils;
mod error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    init_logger();
    let terminal = App::parse();
    let config = ReefSquidConfig::new(terminal.global.dotenv);
    

    match &terminal.command {

        Commands::Test ( _  ) => {

            test_main(&config).await?;
        }
    }

    Ok(())

}

async fn test_main(config: &ReefSquidConfig) -> Result<(), Box<dyn std::error::Error>> {

    log::info!("Reefsquid configuration initiated: {}", config);
    log::info!("Running test function for development ...");    

    log::info!("Reading localhost certificate and key for TLS connection to MinKnow API");    

    // Path must be volume'd into the Docker container, see the compose file:

    let cert = std::fs::read_to_string("/opt/ont/minknow/conf/rpc-certs/ca.crt")?;
    let ca = Certificate::from_pem(cert);

    // MinKnow API in Python sets the GRPC_CHANNEL_OPTIONS when connecting to the gRPC server securely.
    // This includes the configuration and comment:
    // 
    //      ("grpc.ssl_target_name_override", "localhost")  # that's what our cert's CN is [common name]  <-- NOT NECESSARY, CERTIFICATE ISSUE IN RUSTLS
    //
    // An issue by BostonDynamics on the `tonic` crate returns some weirder errors than what occurr here,
    // but they discuss setting the same SSL target name override option through 'origin':
    //
    //      https://github.com/hyperium/tonic/issues/1033
    //
    // What we are getting through `hyper` is:
    // 
    //      error: InvalidCertificate(Other(UnsupportedCertVersion)) 
    //
    // The error code from the `webpki` library states:
    //
    //      The certificate is not a v3 X.509 certificate.
    //
    // ... taking a note now because it's been hours.
    //
    // This is relevant: https://github.com/rustls/rustls/issues/127
    //
    // I can see two options:
    //
    //      1. Modify the ClientTlsConfig in `tonic` 
    //      2. or replace certificates for MinKnow entirely?
    //
    // The insane thing is that this actually worked thanks to the commenter in the issue... 
    // Holy fuck, what a hack. It now requires a modified version of `tonic`, thorough
    // description and fork of `tonic` that implements this to follow.

    let tls_config = ClientTlsConfig::new().domain_name("localhost").ca_certificate(ca);

    let channel = Channel::from_static("https://localhost:9502")
        .tls_config(tls_config)?
        .connect()
        .await?;

    let mut client = ManagerServiceClient::new(channel);

    log::info!("Requesting version from ManagerService");
    let request = tonic::Request::new(GetVersionInfoRequest {});
    let response = client.get_version_info(request).await?;

    log::info!("Response: {:?}", response);

    Ok(())

}
