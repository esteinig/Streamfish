#![allow(dead_code)]
#![allow(non_camel_case_types)]

use clap::Parser;

use crate::utils::init_logger;
use crate::config::ReefsquidConfig;
use crate::terminal::{App, Commands};
use crate::client::minknow::MinKnowClient;
use crate::client::readuntil::ReadUntilClient;
use crate::server::dori::{DoriServer, DoriClient};
use crate::terminal::{TestDoriArgs, TestReadUntilArgs};
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
    let config = ReefsquidConfig::new(terminal.global.dotenv);
    log::info!("Reefsquid configuration initiated: {}", config);

    match &terminal.command {

        Commands::TestReadUntil ( args  ) => {

            test_read_until(&config, args).await?;
        },
        Commands::TestDori ( args  ) => {

            test_dori_client(&config, args).await?;

        },
        Commands::DoriServer ( _  ) => {

            DoriServer::run(&config.dori).await?;

        },
        Commands::AddDevice ( args  ) => {

            let mut minknow_client = MinKnowClient::connect(&config.minknow).await?;
            minknow_client.clients.manager.add_simulated_device(
                &args.name, SimulatedDeviceType::from_cli(&args.r#type)
            ).await?;
        },
        Commands::RemoveDevice ( args  ) => {

            let mut minknow_client = MinKnowClient::connect(&config.minknow).await?;
            minknow_client.clients.manager.remove_simulated_device(&args.name).await?;
        }
    }

    Ok(())

}

async fn test_read_until(config: &ReefsquidConfig, args: &TestReadUntilArgs) -> Result<(), Box<dyn std::error::Error>> {

    log::info!("Reefsquid configuration initiated: {}", config);

    let mut client = ReadUntilClient::connect(&config).await?;
    client.run("MS12345", &args.channel_start, &args.channel_end).await?;

    Ok(())

}

async fn test_dori_client(config: &ReefsquidConfig, _: &TestDoriArgs) -> Result<(), Box<dyn std::error::Error>> {

    log::info!("Reefsquid configuration initiated: {}", config);

    // let mk = MinKnowClient::connect(&config.minknow).await?;
    // mk.stream_channel_states_queue_log("MS12345", 1, 512).await?;

    let mut client = DoriClient::connect(&config.dori).await?;
    
    client.test_basecall_dorado().await?;

    Ok(())

}

