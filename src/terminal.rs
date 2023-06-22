use clap::{Args, Parser, Subcommand};

/// Reefsquid: a squishy adaptive sampling client
#[derive(Debug, Parser)]
#[command(author, version, about)]
#[clap(name = "reefsquid", version)]
pub struct App {
    #[clap(flatten)]
    pub global: GlobalOptions,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Test configuration and connections
    Test(TestArgs),
    /// Add a simulated device to MinKnow
    AddDevice(AddDeviceArgs),
    /// Remove a simulated device from MinKnow
    RemoveDevice(RemoveDeviceArgs),
}

#[derive(Debug, Args)]
pub struct TestArgs {
   /// Channel start for adaptive sampling
   #[clap(long, short, default_value="1")]
   pub channel_start: u32,
   /// Channel end for adaptive sampling
   #[clap(long, short, default_value="512")]
   pub channel_end: u32,
}

#[derive(Debug, Args)]
pub struct AddDeviceArgs {
   
    /// Name of device to add, restricted by device type
    #[clap(long, short, default_value="MS12345")]
    pub name: String,

    /// Device name restricted by device type
    #[clap(long, short, default_value="minion", value_parser=clap::builder::PossibleValuesParser::new(["minion", "promethion", "p2"]))]
    pub r#type: String,
}


#[derive(Debug, Args)]
pub struct RemoveDeviceArgs {
    /// Name of device to remove
    #[clap(long, short, default_value="MS12345")]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct GlobalOptions {

    /// Verbosity level (can be specified multiple times)
    #[clap(long, short, global = true, default_value="0")]
    pub verbose: usize,

     /// Environmental variable file (.env) for configuration.
     /// 
     /// File must be in current directory tree.
     #[clap(long, short, global = true, default_value="true")]
     pub dotenv: bool,
}
