use clap::Parser;
use crate::terminal::{App, Commands};
use crate::config::ReefSquidConfig;
use crate::utils::init_logger;

mod terminal;
mod config;
mod utils;
mod error;

fn main()  {

    init_logger();
    
    let terminal = App::parse();

    // Initiate configuration from environment variables or dotenv file
    let config = ReefSquidConfig::new(terminal.global.dotenv);
    
    match &terminal.command {

        Commands::Test ( _  ) => {

            log::info!("Configuration initiated: {:?}", config)
            
        }
    }

}