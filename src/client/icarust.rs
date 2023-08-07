//! Icarust simulation runner for benchmarks and testing

use std::path::PathBuf;
use icarust::icarust::Icarust;
use icarust::config::load_toml;
use crate::config::StreamfishConfig;
use serde::{Deserialize, Serialize};

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
    pub commit: String,
    pub description: String,
    pub streamfish_config: PathBuf,
    pub icarust_config: PathBuf,
    pub group: Vec<BenchmarkGroup>
}

impl StreamfishBenchmark {
    pub fn from_toml(file: PathBuf) -> Self {
        let toml_str = std::fs::read_to_string(file).expect("Failed to open IcarustBenchmark configuration file");
        let config: StreamfishBenchmark = toml::from_str(&toml_str).expect("Failed to parse IcarustBenchmark configuration file");
        config
    }
    // Inititate the benchmark by creating directories and configs
    pub fn configure(&self, force: bool) -> Vec<(BenchmarkGroup, Benchmark, StreamfishConfig)> {

        let streamfish_config = StreamfishConfig::from_toml(&self.streamfish_config).expect("Failed to parse Streamfish configuration for benchmarks");
        let icarust_config = load_toml(&self.icarust_config);

        log::info!("Creating benchmark directory: {}", &self.outdir.display());
        if self.outdir.exists() && !force {
            log::error!("Benchmark run directory already exists");
            std::process::exit(1)
        } else {
            std::fs::create_dir_all(self.outdir.clone()).expect("Failed to create benchmark directory");
        }
        
        let groups = self.group.clone();
        let mut configured_benchmarks = Vec::new();

        let streamfish_cfg = streamfish_config.clone();
        for mut group in groups {

            log::info!("Benchmark group: prefix={}", &group.prefix);
            
            // Create the benchmark output directories using their prefixes
            let group_dir = self.outdir.join(&group.prefix);
            if group_dir.exists() && !force {
                log::error!("Benchmark group directory already exists");
                std::process::exit(1)
            } else {
                std::fs::create_dir_all(&group_dir).expect("Failed to create benchmark group directory");
            }
            group.path = group_dir.clone();

            let streamfish_cfg = streamfish_cfg.clone();
            let grp = group.clone();

            for mut benchmark in group.benchmark {

                benchmark.uuid = uuid::Uuid::new_v4().to_string();
                log::info!("Benchmark: prefix={} uuid={}", benchmark.prefix, benchmark.uuid);

                let benchmark_dir = group_dir.join(&benchmark.prefix);
                if benchmark_dir.exists() && !force {
                    log::error!("Benchmark directory already exists");
                    std::process::exit(1)
                } else {
                    std::fs::create_dir_all(&benchmark_dir).expect("Failed to create benchmark directory");
                }
                benchmark.path = benchmark_dir.clone();

                // Create a mutable clone of the Streamfish and Icarust configuratiosn for each benchmark
                let mut benchmark_streamfish = streamfish_cfg.clone();
                let mut benchmark_icarust = icarust_config.clone();

                if let Some(unblock_all_mode) = &benchmark.unblock_all_mode {
                    benchmark_streamfish.readuntil.unblock_all = true;
                    benchmark_streamfish.readuntil.unblock_all_mode = unblock_all_mode.clone();
                    log::info!("Configured benchmark [unblock_all_mode={unblock_all_mode}]")
                }

                if let Some(guppy_model) = &benchmark.guppy_model {
                    benchmark_streamfish.guppy.server.config = format!("{guppy_model}.cfg");
                    benchmark_streamfish.guppy.client.config = guppy_model.clone();
                    log::info!("Configured benchmark [guppy_model={guppy_model}]")
                }

                if let Some(reference) = &benchmark.reference {
                    benchmark_streamfish.experiment.reference = reference.clone();
                    log::info!("Configured benchmark [reference={}]", reference.display())
                }                

                if let Some(read_cache_max_chunks) = benchmark.read_cache_max_chunks {
                    benchmark_streamfish.readuntil.read_cache_max_chunks = read_cache_max_chunks;
                    log::info!("Configured benchmark [read_cache_max_chunks={read_cache_max_chunks}]")
                }        

                if let Some(channels) = &benchmark.channels {
                    // No slice and dice configuration
                    benchmark_streamfish.readuntil.channels = channels.to_owned();
                    benchmark_streamfish.readuntil.channel_start = 1;
                    benchmark_streamfish.readuntil.channel_end = channels.to_owned();
                    benchmark_icarust.parameters.channels = channels.to_owned() as usize;
                    log::info!("Configured benchmark [channels={channels}]")

                }                
                
                // Adjust the mapper and basecaller commands
                benchmark_streamfish.experiment.configure();
                benchmark_streamfish.configure();


                // Write to configs into the benchmark directory
                log::info!("Writing configurations to benchmark directory");

                let icarust_config = benchmark_dir.join(format!("{}.icarust.json", &benchmark.prefix));
                benchmark_icarust.to_json(&icarust_config);

                // Set the Icarust config path for the benchmark in the StreamfishConfig
                benchmark_streamfish.icarust.config = icarust_config;

                benchmark_streamfish.to_json(&benchmark_dir.join(format!("{}.streamfish.json", &benchmark.prefix)));
                benchmark.to_json(&benchmark_dir.join(format!("{}.benchmark.json", &benchmark.prefix)));

                configured_benchmarks.push((grp.clone(), benchmark.clone(), benchmark_streamfish.clone()));
            }
        }
        configured_benchmarks
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BenchmarkGroup {

    #[serde(skip_deserializing)]
    pub path: PathBuf,

    pub prefix: String,
    pub description: Option<String>,

    pub benchmark: Vec<Benchmark>
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Benchmark {

    #[serde(skip_deserializing)]
    pub uuid: String,
    #[serde(skip_deserializing)]
    pub path: PathBuf,


    pub prefix: String,
    pub description: Option<String>,
    
    // Benchmarkable parameter fields

    pub channels: Option<u32>,                // Benchmark throughput [Streamfish, Icarust]
    pub guppy_model: Option<String>,            // Benchmark Guppy models
    pub unblock_all_mode: Option<String>,       // Benchmark unblock all stages
    pub read_cache_max_chunks: Option<usize>,            // Benchmark maximum chunks in cache
    pub reference: Option<PathBuf>,             // Benchmark experiment references
}
impl Benchmark {
    pub fn to_json(&self, file: &PathBuf) {
        serde_json::to_writer(
            &std::fs::File::create(&file).expect("Faile to create Benchmark configuration file"), &self
        ).expect("Failed to write Benchmark configuration to file")
    }
}
