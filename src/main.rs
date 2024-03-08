#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unreachable_code)]

use clap::Parser;
use client::icarust::StreamfishBenchmark;
use config::SliceDiceConfig;
use crate::utils::init_logger;
use crate::error::StreamfishError;
use crate::server::dori::DoriServer;
use crate::config::{StreamfishConfig, StreamfishConfigArgs, ServerType};
use crate::terminal::{App, Commands, EvaluateCommands};
use crate::client::minknow::MinknowClient;
use crate::client::readuntil::ReadUntilClient;
use crate::services::minknow_api::manager::SimulatedDeviceType;
use crate::client::readuntil::Termination;
use crate::evaluation::tools::EvaluationTools;

use crate::server::watcher;
mod evaluation;
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

            let config_args = Some(
                StreamfishConfigArgs::new(
                    args.control, 
                    args.dynamic,
                    args.debug_mapping,
                    args.outdir.clone(),
                    args.prefix.clone(),
                    args.simulation.clone(),
                    args.reference.clone(),
                    args.basecaller_model.clone(),
                    args.seed,
                )
            );

            let client = ReadUntilClient::new();
            let config = StreamfishConfig::from_toml(&args.config, config_args)?;
            
            match &args.slice_dice {
                Some(slice_config) => {
                    log::info!("Launched slice-and-dice adaptive sampling routine...");
                    let slice_cfg = SliceDiceConfig::from_toml(&slice_config)?;
                    client.run_slice_dice(&config, &slice_cfg).await?;
                },
                None => {
                    if config.readuntil.read_cache {
                        log::info!("Launched cache-enabled adaptive sampling routine...");
                        client.run_cached(config, None, Termination::ProcessExit).await?;
                    } else {
                        unimplemented!("Streaming RPC not implemented")
                    }
        
                }
            }
        },

        Commands::Watch ( args ) => {
          
            if let Err(error) = watcher::watch_production(
                &args.path, 
                std::time::Duration::from_secs(args.interval),
                std::time::Duration::from_secs(args.timeout),
                std::time::Duration::from_secs(args.timeout_interval)
            ) {
                log::error!("Error: {error:?}");
            }
            

        },
        Commands::Benchmark ( args ) => {
            
            let client = ReadUntilClient::new();
            let config = StreamfishBenchmark::from_toml(&args.config)?;

            client.run_benchmark(&config, args.force, Termination::ProcessExit).await?;

        },
        Commands::DoriServer ( args ) => {

            let config = StreamfishConfig::from_toml(&args.config, None)?;
            DoriServer::run(config, ServerType::from(&args.server_type)).await?;
        },

        Commands::Evaluate ( subcommand ) => {

            let tools = EvaluationTools::new();

            match subcommand {
                EvaluateCommands::Cipher(args) => {
                    tools.cipher_timeseries(&args.directory, &args.metadata, &args.output)?;
                }
            };
        },

        Commands::AddDevice ( args  ) => {

            let config = StreamfishConfig::from_toml(&args.config, None)?;
            let mut minknow_client = MinknowClient::connect(&config.minknow, None).await?;
            minknow_client.clients.manager.add_simulated_device(&args.name, SimulatedDeviceType::from_cli(&args.r#type)).await?;
        },
        Commands::RemoveDevice ( args  ) => {

            let config = StreamfishConfig::from_toml(&args.config, None)?;
            let mut minknow_client = MinknowClient::connect(&config.minknow, None).await?;
            minknow_client.clients.manager.remove_simulated_device(&args.name).await?;
        }
    }

    Ok(())

}


