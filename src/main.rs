#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unreachable_code)]

use clap::Parser;
use client::icarust::StreamfishBenchmark;
use config::SliceDiceConfig;
use crate::utils::init_logger;
use crate::error::StreamfishError;
use crate::server::dori::DoriServer;
use crate::config::{StreamfishConfig, ServerType};
use crate::terminal::{App, Commands};
use crate::client::minknow::MinknowClient;
use crate::client::readuntil::ReadUntilClient;
use crate::services::minknow_api::manager::SimulatedDeviceType;
use crate::client::readuntil::Termination;
mod terminal;
mod services;
mod server;
mod client;
mod config;
mod utils;
mod error;

#[tokio::main]
async fn main() -> Result<(), StreamfishError> {

    init_logger();

    let terminal = App::parse();

    match &terminal.command {
        Commands::ReadUntil ( args ) => {

            let config = StreamfishConfig::from_toml(&args.config)?;
            let client = ReadUntilClient::new();
            
            match &args.slice_dice {
                Some(slice_config) => {
                    let slice_cfg = SliceDiceConfig::from_toml(&slice_config)?;
                    client.run_slice_dice(&config, &slice_cfg).await?;
                },
                None => {
                    if config.readuntil.read_cache {
                        client.run_cached(config, None, Termination::ProcessExit).await?;
                    } else {
                        unimplemented!("Streaming RPC not implemented")
                    }
        
                }
            }
        },

        Commands::Benchmark ( args ) => {
            
            let config = StreamfishBenchmark::from_toml(&args.config)?;
            let client = ReadUntilClient::new();

            client.run_benchmark(&config, args.force, Termination::ProcessExit).await?;

        },
        Commands::DoriServer ( args ) => {

            let config = StreamfishConfig::from_toml(&args.config)?;
            DoriServer::run(config, ServerType::from(&args.server_type)).await?;
        },
        Commands::AddDevice ( args  ) => {

            let config = StreamfishConfig::from_toml(&args.config)?;
            let mut minknow_client = MinknowClient::connect(&config.minknow, None).await?;
            minknow_client.clients.manager.add_simulated_device(&args.name, SimulatedDeviceType::from_cli(&args.r#type)).await?;
        },
        Commands::RemoveDevice ( args  ) => {

            let config = StreamfishConfig::from_toml(&args.config)?;
            let mut minknow_client = MinknowClient::connect(&config.minknow, None).await?;
            minknow_client.clients.manager.remove_simulated_device(&args.name).await?;
        }
    }

    Ok(())

}


