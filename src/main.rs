#![allow(dead_code)]

use clap::Parser;
use crate::utils::init_logger;
use crate::terminal::{App, Commands};


use crate::config::ReefsquidConfig;
use crate::clients::minknow::MinknowClient;
use crate::services::minknow_api::manager::SimulatedDeviceType;

mod services;
mod terminal;
mod clients;
mod config;
mod utils;
mod error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    init_logger();

    let terminal = App::parse();
    let config = ReefsquidConfig::new(terminal.global.dotenv);
    

    match &terminal.command {

        Commands::Test ( _  ) => {

            test_main(&config).await?;
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

async fn test_main(config: &ReefsquidConfig) -> Result<(), Box<dyn std::error::Error>> {

    log::info!("Reefsquid configuration initiated: {}", config);
    log::info!("Running test function for development ...");     

    let minknow_client = MinknowClient::connect(&config.minknow).await?;
    

    Ok(())

}
