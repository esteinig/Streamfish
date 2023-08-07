//! Icarust simulation runner for benchmarks and testing

use std::path::PathBuf;
use icarust::icarust::Icarust;
use icarust::config::Config as IcarustRunConfig;
use crate::config::StreamfishConfig;
use serde::Deserialize;

// Configure and run Icarust
pub struct IcarustRunner {
    pub icarust: Icarust,
    pub config: StreamfishConfig
}
impl IcarustRunner {
    pub fn new(config: &StreamfishConfig) -> Self {

        // Streamfish and Icarust are configured in two distinct files
        // we need to do some checks to ensure that settings are
        // matching between the configurations

        let icarust = Icarust::from_toml(&config.icarust.config);

        if icarust.config.parameters.channels != config.readuntil.channels as usize {
            log::error!("IcarustRunner: channel sizes of Icarust configuration ({}) and Streamfish ReadUntil configuration ({}) do not match", icarust.config.parameters.channels, config.readuntil.channels);
            std::process::exit(1);
        }

        if icarust.config.parameters.manager_port != config.icarust.manager_port {
            log::error!("IcarustRunner: manager port of Icarust configuration ({}) and Streamfish Icarust configuration ({}) do not match", icarust.config.parameters.manager_port, config.icarust.manager_port );
            std::process::exit(1);
        }

        if icarust.config.parameters.position_port != config.icarust.position_port {
            log::error!("IcarustRunner: position port of Icarust configuration ({}) and Streamfish Icarust configuration ({}) do not match", icarust.config.parameters.position_port, config.icarust.position_port );
            std::process::exit(1);
        }

        Self { icarust, config: config.clone() }
    }
}

// A benchmark configuration running Streamfish and Icarust variations
#[derive(Debug, Clone, Deserialize)]
pub struct StreamfishBenchmark {
    pub name: String,
    pub outdir: PathBuf,
    pub date: String,
    pub description: String,
    pub benchmark: Vec<Benchmark>
}

impl StreamfishBenchmark {
    pub fn new(name: String, outdir: PathBuf, date: String, description: String, benchmark: Vec<Benchmark>) -> Self {
        Self { name, outdir, date, description, benchmark }
    }
    pub fn from_toml(file: PathBuf) -> Self {
        let toml_str = std::fs::read_to_string(file).expect("Failed to open IcarustBenchmark configuration file");
        let config: StreamfishBenchmark = toml::from_str(&toml_str).expect("Failed to parse IcarustBenchmark configuration file");
        config
    }
    pub async fn run() {

    }
}


#[derive(Debug, Clone, Deserialize)]
pub struct Benchmark {

    #[serde(skip_deserializing)]
    pub uuid: String,

    pub prefix: String,
    pub description: Option<String>,
    
    // Benchmarkable parameter fields

    pub channels: Option<Vec<usize>>,                // Benchmark throughput [Streamfish, Icarust]
    pub guppy_model: Option<Vec<String>>,            // Benchmark Guppy models
    pub unblock_all_mode: Option<Vec<String>>,       // Benchmark unblock all stages
    pub max_chunk_size: Option<Vec<u32>>,            // Benchmark maximum chunks in cache
    pub reference: Option<Vec<PathBuf>>,             // Benchmark experiment references
    pub targets: Option<Vec<Vec<String>>>,           // Benchmark experiment targets
}


// A single run config with modified Icarust/Streamfish configs
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub uuid: String,
    
    pub streamfish: StreamfishConfig,
    pub icarust: IcarustRunConfig,

    // Benchmarkable parameter fields

    pub channels: Option<usize>,                // Benchmark throughput [Streamfish, Icarust]
    pub guppy_model: Option<String>,            // Benchmark Guppy models
    pub unblock_all_mode: Option<String>,       // Benchmark unblock all stages
    pub max_chunk_size: Option<u32>,            // Benchmark maximum chunks in cache
    pub reference: Option<PathBuf>,             // Benchmark experiment references
    pub targets: Option<Vec<String>>,           // Benchmark experiment targets

}