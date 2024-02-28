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
    /// Input simulation (.blow5) from Cipher/Squigulator overrides config input for Icarust
    #[clap(long, short)]
    pub simulation: Option<PathBuf>,
    /// TOML configuration file for slice-and-dice operation with Streamfish
    #[clap(long)]
    pub slice_dice: Option<PathBuf>,
    /// Output directory for read data from Icarust simulation
    #[clap(long, short)]
    pub outdir: Option<PathBuf>,
    /// Run the experiment in control mode - every action request is StopFurtherData
    #[clap(long)]
    pub control: bool,
    /// Enable the dynamic adaptive sampling loop configuration
    #[clap(long, short)]
    pub dynamic: bool,
    /// Debug read mapping and action requests - introduces significant latency, do not use for experiments!
    #[clap(long)]
    pub debug_mapping: bool,
    /// Optional prefix for Icarust simulation output files
    #[clap(long)]
    pub prefix: Option<String>,
    /// Seed default set to 0 which generates new seed value for every run execution
    #[clap(long, default_value="0")]
    pub seed: u64,
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
    /// Cipher signal read file for meta data linkage 
    #[clap(long, short)]
    pub read_summary: PathBuf,
    
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
