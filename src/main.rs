#![allow(dead_code)]
#![allow(non_camel_case_types)]

use clap::Parser;

use crate::utils::init_logger;
use crate::config::StreamfishConfig;
use crate::terminal::{App, Commands};
use crate::client::minknow::MinKnowClient;
use crate::client::readuntil::ReadUntilClient;
use crate::server::dori::DoriServer;
use crate::terminal::TestReadUntilArgs;
use crate::services::minknow_api::manager::SimulatedDeviceType;

mod terminal;
mod services;
mod server;
mod client;
mod config;
mod utils;
mod error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    init_logger();

    let terminal = App::parse();
    let mut config = StreamfishConfig::new(terminal.global.config);

    match &terminal.command {
        Commands::TestReadUntil ( args  ) => {

            test_read_until(&mut config, args).await?;
        },
        Commands::DoriServer ( args ) => {

            config.cli_config(
                None,
                None,
                args.dori_socket.clone(),
                args.guppy_address.clone(),
                None, 
                None
            );

            DoriServer::run(&config).await?;

        },
        Commands::AddDevice ( args  ) => {

            let mut minknow_client = MinKnowClient::connect(&config.minknow, &config.icarust).await?;
            minknow_client.clients.manager.add_simulated_device(&args.name, SimulatedDeviceType::from_cli(&args.r#type)).await?;
        },
        Commands::RemoveDevice ( args  ) => {

            let mut minknow_client = MinKnowClient::connect(&config.minknow, &config.icarust).await?;
            minknow_client.clients.manager.remove_simulated_device(&args.name).await?;
        }
    }

    Ok(())

}

async fn test_read_until(config: &mut StreamfishConfig, args: &TestReadUntilArgs) -> Result<(), Box<dyn std::error::Error>> {

    config.cli_config(
        args.channel_start, 
        args.channel_end,
        args.dori_socket.clone(),
        None,
        args.dori_port,
        args.log_latency.clone(),

    );

    let mut client = ReadUntilClient::connect(config).await?;
    
    if config.readuntil.read_cache && config.readuntil.read_cache_batch_rpc {
        client.run_dorado_cache_batch().await?;
    } else if config.readuntil.read_cache && !config.readuntil.read_cache_batch_rpc {
        client.run_dorado_cache_channel().await?;
    } else {
        unimplemented!("Pure streaming RPC not implemented on this branch")
    }

    Ok(())

}


