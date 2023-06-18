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
}

#[derive(Debug, Args)]
pub struct TestArgs {
   
}

#[derive(Debug, Args)]
pub struct GlobalOptions {

    /// Verbosity level (can be specified multiple times)
    #[clap(long, short, global = true, default_value="0")]
    pub verbose: usize,

     /// use an nvironmental variable file (.env) for configuration.
     /// 
     /// File must be in current directory tree.
     #[clap(long, short, global = true, default_value="true")]
     pub dotenv: bool,
}
