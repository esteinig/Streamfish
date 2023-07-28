#![allow(dead_code)]
#![allow(non_camel_case_types)]

use clap::Parser;
use config::SliceDiceConfig;
use crate::utils::init_logger;
use crate::error::StreamfishError;
use crate::server::dori::DoriServer;
use crate::config::StreamfishConfig;
use crate::terminal::{App, Commands};
use crate::tools::minimap::Minimapper;
use crate::client::minknow::MinknowClient;
use crate::client::readuntil::ReadUntilClient;
use crate::services::minknow_api::manager::SimulatedDeviceType;

mod terminal;
mod services;
mod server;
mod client;
mod config;
mod utils;
mod error;
mod tools;

#[tokio::main]
async fn main() -> Result<(), StreamfishError> {

    init_logger();

    let terminal = App::parse();

    match &terminal.command {
        Commands::ReadUntil ( _  ) => {

            let config = StreamfishConfig::from_toml(terminal.global.config)?;
            let client = ReadUntilClient::new();
            
            match terminal.global.slice_dice {
                Some(slice_config) => {
                    let slice_cfg = SliceDiceConfig::from_toml(&slice_config)?;
                    client.run_slice_dice(&config, &slice_cfg).await?;
                },
                None => {

                    if config.readuntil.read_cache {
                        client.run_cached(config).await?;
                    } else {
                        unimplemented!("Streaming RPC not implemented")
                    }
        
                }
            }

          
        },
        Commands::DoriServer ( _ ) => {

            let config = StreamfishConfig::from_toml(terminal.global.config)?;
            DoriServer::run(config).await?;
        },
        Commands::AddDevice ( args  ) => {
            let config = StreamfishConfig::from_toml(terminal.global.config)?;

            let mut minknow_client = MinknowClient::connect(&config.minknow, &config.icarust).await?;
            minknow_client.clients.manager.add_simulated_device(&args.name, SimulatedDeviceType::from_cli(&args.r#type)).await?;
        },
        Commands::RemoveDevice ( args  ) => {
            let config = StreamfishConfig::from_toml(terminal.global.config)?;

            let mut minknow_client = MinknowClient::connect(&config.minknow, &config.icarust).await?;
            minknow_client.clients.manager.remove_simulated_device(&args.name).await?;
        },
        Commands::MinimapRs ( args ) => {
            let mm = Minimapper::new(args.index.clone());
            mm.map(args.fastx.clone(), args.threads);
        },
    }

    Ok(())

}


