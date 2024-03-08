use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};

/// Streamfish: a streamy adaptive sampling client
#[derive(Debug, Parser)]
#[command(author, version, about)]
#[command(styles=get_styles())]
#[command(arg_required_else_help(true))]
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
    ReadUntil(ReadUntilArgs),
    /// Benchmark client
    Benchmark(BenchmarkArgs),
    /// Dori server
    DoriServer(DoriServerArgs),
    /// Add a simulated device to MinKnow
    AddDevice(AddDeviceArgs),
    /// Remove a simulated device from MinKnow
    RemoveDevice(RemoveDeviceArgs),
    /// Test the watcher service for launching Nextflow pipelines
    Watch(WatcherArgs),
    #[clap(subcommand)]
    /// Evaluation subcommands
    Evaluate(EvaluateCommands),
}

#[derive(Debug, Args)]
pub struct DoriServerArgs {
    /// TOML configuration file for Streamfish
    #[clap(long, short)]
    pub config: PathBuf,
    /// Server type - adaptive or dynamic
    #[clap(long, short, default_value="adaptive", value_parser=clap::builder::PossibleValuesParser::new(["adaptive", "dynamic"]))]
    pub server_type: String,
    
 }

#[derive(Debug, Args)]
pub struct ReadUntilArgs {
    /// TOML configuration file for Streamfish
    #[clap(long, short)]
    pub config: PathBuf,
    /// Output directory for read data from Icarust simulation
    #[clap(long, short)]
    pub outdir: Option<PathBuf>,
    /// Input simulation (.blow5) from Cipher/Squigulator overrides config input for Icarust
    #[clap(long, short)]
    pub simulation: Option<PathBuf>,
    /// Run the experiment in control mode - every action request is StopFurtherData
    #[clap(long)]
    pub control: bool,
    /// Enable the dynamic adaptive sampling loop configuration
    #[clap(long, short)]
    pub dynamic: bool,
    /// Reference index for a globally configured e.g. `targeted_sequencing` or `host_depletion` experiment preset
    #[clap(long, short)]
    pub reference: Option<PathBuf>,
    #[clap(long, short)]
    /// Basecaller model for the Guppy or Dorado basecaller server config and request data
    pub basecaller_model: Option<String>,
    /// Debug read mapping and action requests - introduces significant latency, do not use for experiments!
    #[clap(long)]
    pub debug_mapping: bool,
    /// Optional prefix for Icarust simulation output files
    #[clap(long)]
    pub prefix: Option<String>,
    /// Seed default set to 0 which generates new seed value for every run execution
    #[clap(long, default_value="0")]
    pub seed: u64,
    /// TOML configuration file for slice-and-dice operation with Streamfish
    #[clap(long)]
    pub slice_dice: Option<PathBuf>,
 }



 #[derive(Debug, Args)]
 pub struct WatcherArgs {
     /// File path to watch recursively for input folders
     #[clap(long, short = 'p', default_value=".")]
     pub path: PathBuf,
     /// Interval for polling file path recursively in seconds
     #[clap(long, short = 'i', default_value="3")]
     pub interval: u64,
     /// Timeout in seconds to proceed after no further events on input folder
     #[clap(long, short = 't', default_value="10")]
     pub timeout: u64,
     /// Timeout interval for polling input folder recursively in seconds
     #[clap(long, short = 'm', default_value="1")]
     pub timeout_interval: u64,
 }

#[derive(Debug, Args)]
pub struct BenchmarkArgs {
    /// TOML configuration file for Streamfish benchmark
    #[clap(long, short)]
    pub config: PathBuf,
    /// Force overwrite the benchmark directories
    #[clap(long, short)]
    pub force: bool,
    
}


#[derive(Debug, Args)]
pub struct AddDeviceArgs {
    /// TOML configuration file for Streamfish
    #[clap(long, short)]
    pub config: PathBuf,
    /// Name of device to add, restricted by device type
    #[clap(long, short, default_value="MS12345")]
    pub name: String,
    /// Device name restricted by device type
    #[clap(long, short, default_value="minion", value_parser=clap::builder::PossibleValuesParser::new(["minion", "promethion", "p2"]))]
    pub r#type: String,
}

#[derive(Debug, Args)]
pub struct RemoveDeviceArgs {
    /// TOML configuration file for Streamfish.
    #[clap(long, short)]
    pub config: PathBuf,
    /// Name of device to remove
    #[clap(long, short, default_value="MS12345")]
    pub name: String,
}

#[derive(Debug, Args)]
pub struct GlobalOptions {
    /// Verbosity level (can be specified multiple times)
    #[clap(long, short, global = true, default_value="0")]
    pub verbose: usize,
}


/* EVALUATIONS AND TRANSFORMATIONS */

#[derive(Debug, Subcommand)]
pub enum EvaluateCommands {
    /// Create a read time series table from adaptive
    /// sampling outputs (*.blow5) that links read  
    /// identifiers to community member meta data
    /// from the Cipher community simulation
    Cipher(CipherArgs),
}   


#[derive(Debug, Args)]
pub struct CipherArgs {
    /// Directory with Blow5 output files from a run
    #[clap(long, short)]
    pub directory: PathBuf,
    /// Output table of time-series signal reads linked
    /// to meta-data of the community simulation
    #[clap(long, short)]
    pub output: PathBuf,
    /// Cipher signal read file for metadata linkage 
    #[clap(long, short)]
    pub metadata: PathBuf,
    
 }



pub fn get_styles() -> clap::builder::Styles {
	clap::builder::Styles::styled()
        .error(
            anstyle::Style::new()
            .bold()
            .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
        .invalid(
            anstyle::Style::new()
            .bold()
            .fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red))),
        )
		.header(
			anstyle::Style::new()
				.underline()
				.fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
		)
        .usage(
			anstyle::Style::new()
				.underline()
				.fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Yellow))),
		)
		.literal(
			anstyle::Style::new()
				.fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green))),
		)
}
