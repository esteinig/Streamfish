use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};

/// Streamfish: a streamy adaptive sampling client
#[derive(Debug, Parser)]
#[command(author, version, about)]
#[clap(name = "streamfish", version)]
pub struct App {
    #[clap(flatten)]
    pub global: GlobalOptions,

    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// ReadUntil client
    ReadUntil(TestReadUntilArgs),
    /// Dori server
    DoriServer(DoriServerArgs),
    /// Add a simulated device to MinKnow
    AddDevice(AddDeviceArgs),
    /// Remove a simulated device from MinKnow
    RemoveDevice(RemoveDeviceArgs),
}

#[derive(Debug, Args)]
pub struct DoriServerArgs { }

#[derive(Debug, Args)]
pub struct TestReadUntilArgs { }

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
    /// TOML configuration file for Streamfish.
    #[clap(long, short, global = true, default_value="streamfish.toml")]
    pub config: PathBuf,
    /// TOML configuration file for slice-and-dice operation with Streamfish.
    #[clap(long, short, global = true)]
    pub slice_dice: Option<PathBuf>,
}
