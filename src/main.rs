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
    let mut config = StreamfishConfig::new(terminal.global.dotenv);

    match &terminal.command {

        Commands::TestReadUntil ( args  ) => {

            test_read_until(&mut config, args).await?;
        },
        Commands::DoriServer ( _  ) => {

            DoriServer::run(&config).await?;

        },
        Commands::AddDevice ( args  ) => {

            let mut minknow_client = MinKnowClient::connect(&config.minknow).await?;
            minknow_client.clients.manager.add_simulated_device(&args.name, SimulatedDeviceType::from_cli(&args.r#type)).await?;
        },
        Commands::RemoveDevice ( args  ) => {

            let mut minknow_client = MinKnowClient::connect(&config.minknow).await?;
            minknow_client.clients.manager.remove_simulated_device(&args.name).await?;
        }
    }

    Ok(())

}

async fn test_read_until(config: &mut StreamfishConfig, args: &TestReadUntilArgs) -> Result<(), Box<dyn std::error::Error>> {

    let mut client = ReadUntilClient::connect(config, &args.log_latency).await?;
    client.run().await?;

    Ok(())

}


