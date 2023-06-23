#![allow(dead_code)]

use clap::Parser;
use terminal::TestArgs;
use crate::utils::init_logger;
use crate::terminal::{App, Commands};


use crate::config::ReefsquidConfig;
use crate::client::minknow::{MinknowClient, ReadUntilClient};
use crate::services::minknow_api::manager::SimulatedDeviceType;

mod services;
mod terminal;
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

        Commands::Test ( args  ) => {

            test_main(&config, args).await?;
        },
        Commands::AddDevice ( args  ) => {

            let mut minknow_client = MinknowClient::connect(&config.minknow).await?;
            minknow_client.clients.manager.add_simulated_device(
                &args.name, SimulatedDeviceType::from_cli(&args.r#type)
            ).await?;
        },
        Commands::RemoveDevice ( args  ) => {

            let mut minknow_client = MinknowClient::connect(&config.minknow).await?;
            minknow_client.clients.manager.remove_simulated_device(&args.name).await?;
        }
    }

    Ok(())

}

async fn test_main(config: &ReefsquidConfig, args: &TestArgs) -> Result<(), Box<dyn std::error::Error>> {

    log::info!("Reefsquid configuration initiated: {}", config);

    // let mk = MinknowClient::connect(&config.minknow).await?;
    // mk.stream_channel_states_queue_log("MS12345", 1, 512).await?;

    let mut client = ReadUntilClient::new(&config).await?;
    
    client.run("MS12345", &args.channel_start, &args.channel_end, 0.1, true).await?;

    Ok(())

}
