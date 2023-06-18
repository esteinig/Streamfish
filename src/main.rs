#![allow(dead_code)]

use std::future::Future;

use clap::Parser;

use crate::terminal::{App, Commands};
use crate::config::ReefSquidConfig;
use crate::utils::init_logger;

mod terminal;
mod config;
mod utils;
mod error;
mod api;

#[tokio::main]
async fn main() {

    init_logger();
    let terminal = App::parse();
    let config = ReefSquidConfig::new(terminal.global.dotenv);
    

    match &terminal.command {

        Commands::Test ( _  ) => {

            test_main(&config).await;
        }
    }

}

async fn test_main(config: &ReefSquidConfig){

    log::info!("Reefsquid configuration initiated: {}", config);
    log::info!("Running in async test function for development");    



}

async fn build_proto(config: &ReefSquidConfig){

    log::info!("Reefsquid configuration initiated: {}", config);
    log::info!("Running in async test function for development");    



}