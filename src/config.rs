


use minimap2::Mapping;
use std::path::PathBuf;
use std::io::BufRead;
use rand::{self, Rng};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};

use crate::services::dori_api::dynamic::DynamicTarget;
use crate::{services::{minknow_api::data::get_live_reads_request::RawDataType, dori_api::adaptive::Decision}, error::StreamfishConfigError};

fn get_env_var(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

// Command line overrides on launchin Streamgish Read-Until
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamfishConfigArgs {
    control: bool,
    dynamic: bool,
    debug_mapping: bool,
    outdir: Option<PathBuf>,
    prefix: Option<String>,
    simulation: Option<PathBuf>,
    reference: Option<PathBuf>,
    basecaller_model: Option<String>,
    basecaller_server: Option<PathBuf>,
    experiment_config: Option<PathBuf>,
    seed: u64
}
impl StreamfishConfigArgs {
    pub fn new(
        control: bool, 
        dynamic: bool, 
        debug_mapping: bool, 
        outdir: Option<PathBuf>, 
        prefix: Option<String>, 
        simulation: Option<PathBuf>, 
        reference: Option<PathBuf>, 
        basecaller_model: Option<String>,
        basecaller_server: Option<PathBuf>,
        experiment_config: Option<PathBuf>, 
        seed: u64
    ) -> Self {
        Self {
            control,
            dynamic,
            debug_mapping,
            outdir,
            prefix,
            simulation,
            reference,
            basecaller_model,
            basecaller_server,
            experiment_config,
            seed
        }
    }
}


// An exposed subset of configurable parameters for the user
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamfishConfig  {
    pub meta: MetaConfig,
    pub debug: DebugConfig,
    pub minknow: MinknowConfig,
    pub icarust: IcarustConfig,
    pub basecaller: BasecallerConfig,
    pub dori: DoriConfig,
    pub dynamic: DynamicConfig,
    pub readuntil: ReadUntilConfig,
    pub experiment: ExperimentConfig
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetaConfig {
    pub name: String,
    pub version: String,
    pub description: String,
    pub client_name: String,
    pub server_name: String,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebugConfig {
    pub cache: bool,
    pub mapping: bool
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinknowConfig {
    pub port: i32,
    pub host: String,
    pub token: String,
    pub certificate: PathBuf,
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IcarustConfig {
    pub enabled: bool,
    pub data_seed: u64,
    pub manager_port: u32,
    pub position_port: u32,
    pub config: PathBuf,
    pub outdir: Option<PathBuf>,           // overrides path in configuration 
    pub prefix: Option<String>,            // adds subdir to outdir
    pub simulation: Option<PathBuf>,       // change input simulation on the fly
    pub deplete: bool,                     // should be true for now
    pub launch: bool,
    pub delay: u64,
    pub runtime: u64,
    pub task_delay: u64,
    pub log_actions: bool,

    #[serde(skip_deserializing)]
    pub sample_rate: u32,
}


pub enum ServerType {
    Adaptive,
    Dynamic
}
impl ServerType {
    pub fn from(s: &str) -> Self {
        match s {
            "adaptive" => Self::Adaptive,
            "dynamic" => Self::Dynamic,
            _ => unimplemented!("Server type not implemented: {s}")
        }
    }
}

// Distinct RPC servers are launched for the adaptive
// sampling and the dynamic feedback services - this is
// because the dynamic feedback service is not sensitive
// to latency like the adaptive sampling service, but may 
// require more resources (memory, processors, GPU) for slow
// real-time analysis of the generated data and may therefore
// be run on different hardware on the network that syncs the
// POD5/FASTQ output from the run.

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DoriConfig {
    pub adaptive: DoriAdaptiveConfig,
    pub dynamic: DoriDynamicConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DoriAdaptiveConfig {
    pub tcp_enabled: bool,
    pub tcp_port: u32,
    pub tcp_host: String,           // inside docker to expose must be 0.0.0.0
    pub uds_path: PathBuf,
    pub uds_override: bool,
    pub minknow_host: String,
    pub minknow_port: u32,
    pub classifier: Classifier,
    pub basecaller: Basecaller,
    pub stderr_log: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DoriDynamicConfig {
    pub tcp_enabled: bool,
    pub tcp_port: u32,
    pub tcp_host: String,           // inside docker to expose must be 0.0.0.0
    pub uds_path: PathBuf,
    pub uds_override: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DynamicConfig {
    pub enabled: bool,
    pub launch_server: bool,
    pub interval_seconds: u64,
    pub cache_capacity: u64,
    pub test_targets: Vec<Target>
}


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadUntilConfig {
    pub init_delay: u64,
    pub device_name: String,
    pub channels: u32,              
    pub channel_start: u32,
    pub channel_end: u32,
    pub dori_tcp_host: String,  // can be different if outside of container
    pub dori_tcp_port: u32,    
    pub unblock_all: bool,
    pub unblock_all_mode: String,
    pub unblock_all_chunk_file: PathBuf,
    pub read_cache: bool,
    pub read_cache_ttl: u64,
    pub read_cache_tti: u64,
    pub read_cache_max_capacity: u64,
    pub read_cache_min_chunks: usize,
    pub read_cache_max_chunks: usize,
    pub action_throttle: u64,
    pub latency_log: Option<PathBuf>,
    pub unblock_duration: f64,
    pub sample_minimum_chunk_size: u64,
    pub accepted_first_chunk_classifications: Vec<i32>,
    pub launch_dori_server: bool,
    pub launch_basecall_server: bool,

    #[serde(skip_deserializing)]
    pub unblock_all_client: bool,
    #[serde(skip_deserializing)]
    pub unblock_all_server: bool,
    #[serde(skip_deserializing)]
    pub unblock_all_basecaller: bool,
    #[serde(skip_deserializing)]
    pub unblock_all_mapper: bool,
    #[serde(skip_deserializing, skip_serializing)]
    pub raw_data_type: RawDataType
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasecallerConfig {
    pub client: BasecallerClientConfig,
    pub server: BasecallerServerConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasecallerClientConfig {
    pub path: PathBuf,
    pub script: PathBuf,
    pub address: String,
    pub config: String,
    pub throttle: f32,
    pub threads: u32,
    pub max_reads_queued: u32,

    #[serde(skip_deserializing)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasecallerServerConfig {
    pub path: PathBuf,
    pub port: String,
    pub config: String,
    pub num_callers: u32,
    pub chunks_per_runner: u32,
    pub gpu_runners_per_device: u32,
    pub chunk_size: u32,
    pub threads: u32,
    pub device: String,
    pub log_path: PathBuf,
    pub stderr_log: PathBuf,

    #[serde(skip_deserializing)]
    pub args: Vec<String>,

}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExperimentConfig {
    pub config: Option<PathBuf>,
    pub control: bool,
    pub mode: String,
    pub r#type: String,
    pub targets: Vec<Target>,
    pub target_file: PathBuf,
    pub min_match_len: i32,
    pub reference: PathBuf,

    #[serde(skip_deserializing)]
    pub experiment: Experiment
}


// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> std::io::Result<std::io::Lines<std::io::BufReader<std::fs::File>>>
where P: AsRef<std::path::Path>, {
    let file = std::fs::File::open(filename)?;
    Ok(std::io::BufReader::new(file).lines())
}

/// A target specification for targeted sequencing experiments
/// Add negative checks due to i32 `minimap2-rs` types
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    reference: String,
    start: Option<i32>,  
    end: Option<i32>, 
    name: Option<String> 
}
impl Target {
    pub fn from(s: String) -> Result<Self, StreamfishConfigError> {
        let components = s.split_whitespace().into_iter().collect::<Vec<&str>>();
        match components.len() {
            1 => Ok(Target { reference: components[0].to_string(), start: None, end: None, name: None }),
            4 => {
                    let start = components[1].parse::<i32>().map_err(|_| StreamfishConfigError::TargetBounds(s.clone()))?;
                    let end = components[2].parse::<i32>().map_err(|_| StreamfishConfigError::TargetBounds(s.clone()))?;

                    // O-range triggers full reference target
                    if let (0, 0) = (start, end) {
                        Ok(Target { 
                            reference: components[0].to_string(), 
                            start: None, 
                            end: None,
                            name: Some(components[3].to_string())
                        })
                    } else {
                        Ok(Target { 
                            reference: components[0].to_string(), 
                            start: Some(start), 
                            end: Some(end),
                            name: Some(components[3].to_string())
                        })
                    }
            },
            _ => Err(StreamfishConfigError::TargetFormat(s))    
        }
    }
    pub fn to_dynamic_target(&self) -> DynamicTarget {
        DynamicTarget {
            reference: self.reference.clone(),
            start: self.start, 
            end: self.end,
            name: self.name.clone(),
            alignment_index: None,
            channel_start: None,
            channel_end: None
        }
    } 
    pub fn from_dynamic_target(dynamic_target: &DynamicTarget) -> Self {
        Self {
            reference: dynamic_target.reference.clone(),
            start: dynamic_target.start, 
            end: dynamic_target.end,
            name: dynamic_target.name.clone()
        }
    } 
}
impl<'de> Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        let components = s.split("::").into_iter().collect::<Vec<&str>>();
        match components.len() {
            1 => Ok(Target { reference: components[0].to_string(), start: None, end: None, name: None}),
            4 => Ok(Target { 
                reference: components[0].to_string(), 
                start: Some(components[1].parse::<i32>().map_err(|_| StreamfishConfigError::TargetBounds(s.clone())).map_err(D::Error::custom)?), 
                end: Some(components[2].parse::<i32>().map_err(|_| StreamfishConfigError::TargetBounds(s.clone())).map_err(D::Error::custom)?),
                name: Some(components[3].to_string()) 
            }),
            _ => Err(StreamfishConfigError::TargetFormat(s)).map_err(D::Error::custom)
        }
    }
}

pub struct TargetFile {
    targets: Vec<Target>
}
impl TargetFile {
    pub fn from(path: &PathBuf) -> Result<Self, StreamfishConfigError> {
        let mut targets = Vec::new();
        if let Ok(lines) = read_lines(&path) {
            for line in lines {
                if let Ok(s) = line {
                    targets.push(Target::from(s)?)
                }
            }
        }
        Ok(Self{ targets })
    }
}


/// Unblock all circuits for testing
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum UnblockAll {
    Client,
    Server,
    Basecaller,
    Mapper,
}
impl UnblockAll {
    fn from_str(s: &str) -> Self {
        match s {
            "client" => Self::Client,
            "server" => Self::Server,
            "basecaller" => Self::Basecaller,
            "mapper" => Self::Mapper,
            _ => unimplemented!("Unblock all setting `{}` is not implemented", s)
        }
    }
    // A little clunky but betetr to have bools in configuration for 
    // evaluation in streams than matching enumeration types or values
    fn get_config(&self) -> (bool, bool, bool, bool) {
        match self {
            UnblockAll::Client => (true, false, false, false),
            UnblockAll::Server => (false, true, false, false),
            UnblockAll::Basecaller => (false, false, true, false),
            UnblockAll::Mapper => (false, false, false, true)
        }
    }
}

/// Basecallers
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Basecaller {
    Dorado,
    Guppy
}
impl Basecaller {
    pub fn from_str(s: &str) -> Self {
        match s {
            "dorado" => Basecaller::Dorado,
            "guppy" => Basecaller::Guppy,
            _ => unimplemented!("Basecaller `{}` is not implemented", s)
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Basecaller::Dorado => "dorado",
            Basecaller::Guppy => "guppy"
        }
    }
}
impl<'de> Deserialize<'de> for Basecaller {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "dorado" => Basecaller::Dorado,
            "guppy" => Basecaller::Guppy,
            _ => unimplemented!("Basecaller `{}` is not implemented", s)
        })
    }
}

/// Classifiers
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Classifier {
    Minimap2Rust,
    Kraken2
}
impl Classifier {
    pub fn from_str(s: &str) -> Self {
        match s {
            "minimap2-rs" => Self::Minimap2Rust,
            "kraken2" => Self::Kraken2,
            _ => unimplemented!("Classifier `{}` is not implemented", s)
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Classifier::Minimap2Rust => "minimap2-rs",
            Classifier::Kraken2 => "kraken2"
        }
    }
}
impl<'de> Deserialize<'de> for Classifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "minimap2-rs" => Classifier::Minimap2Rust,
            "kraken2" => Classifier::Kraken2,
            _ => unimplemented!("Classifier `{}` is not implemented", s)
        })
    }
}

impl StreamfishConfig {
    pub fn from_toml(file: &PathBuf, args: Option<StreamfishConfigArgs>) -> Result<Self, StreamfishConfigError> {

        let toml_str = std::fs::read_to_string(file).map_err(|err| StreamfishConfigError::TomlConfigFile(err))?;
        let mut config: StreamfishConfig = toml::from_str(&toml_str).map_err(|err| StreamfishConfigError::TomlConfigParse(err))?;


        // Command line argument override...
        if let Some(args) = args.clone() {
            // ... has to happen before parsing the experiment altgernative TOML
            config.experiment.config = args.experiment_config.clone();
            log::info!("Experiment config path was set to >> {:?} << from the command-line", config.experiment.config);

            // Override basecaller server model configuration
            if let Some(server) = &args.basecaller_server {
                config.basecaller.server.path = server.to_owned();
            }
        }

        // Host ports forwarded allows `docker-compose` to set the host via environmental variable
        config.minknow.host = match get_env_var("STREAMFISH_MINKNOW_HOST") { Some(var) => var, None => config.minknow.host.clone() };

        // Always sample uncalibrated data for now
        config.readuntil.raw_data_type = RawDataType::Uncalibrated;

        // If experiment.config TOML file path is set, use config from file
        if let Some(exp_config) = config.experiment.config {
            let toml_str = std::fs::read_to_string(exp_config).map_err(|err| StreamfishConfigError::TomlConfigFile(err))?;
            let exp_cfg: ExperimentConfig = toml::from_str(&toml_str).map_err(|err| StreamfishConfigError::TomlConfigParse(err))?;
            config.experiment = exp_cfg;
        }

        // If target list by file:
        if !config.experiment.target_file.as_os_str().is_empty() {
            
            if !config.experiment.target_file.exists() {
                return Err(StreamfishConfigError::TargetFileNotFound(config.experiment.target_file.display().to_string()))
            }

            config.experiment.targets = TargetFile::from(&config.experiment.target_file)?.targets;
            log::info!("Loaded targets from file: {:#?}", config.experiment.targets);
        }


        // Configure the experiment and mapping settings
        config.experiment.configure();

        // Configure the basecaller and classifier arguments
        config.configure();

        // Command line argument override...
        if let Some(args) = args {

            config.dynamic.enabled = args.dynamic;
            config.experiment.control = args.control;
            config.icarust.outdir = args.outdir.clone();
            
            if let Some(ref_index) = &args.reference {
                config.experiment.reference = ref_index.to_path_buf();
            }

            let data_seed = match args.seed > 0 {
                true => args.seed,
                false =>    { 
                    let mut rng = rand::thread_rng();
                    rng.gen()
                }
            };
            
            // Used in data sampling configuration (pore /channel strand order) during Icarust fork reader from Cipher (.blow5)
            config.icarust.data_seed = data_seed;

            // => REMOVED <= Debug mappings for experiment tests by logging to console => REMOVED <=
            config.debug.mapping = args.debug_mapping;

            // Prefix for output filers in Icarust writer (.blow5)
            config.icarust.prefix = args.prefix.clone();

            // Simulation input for Icarust and Cipher (.blow5))
            config.icarust.simulation = args.simulation.clone();

            // Override basecaller server model configuration
            if let Some(model) = &args.basecaller_model {

                config.basecaller.client.config = model.clone();

                let mut model_cfg = model.clone();
                model_cfg.push_str(".cfg");

                config.basecaller.server.config = model_cfg;
            }
            

            // Override basecaller server model configuration
            if let Some(model) = &args.basecaller_model {

            }

            log::info!("Command-line arguments override: {:#?}", args);
        }
         
        Ok(config)

    }
}

impl StreamfishConfig {
    // Configure fields that require modifications, basecaller and classifer arguments on the adaptive server
    pub fn configure(&mut self) {
        
        let (unblock_all_client, unblock_all_server, unblock_all_basecaller, unblock_all_mapper) = match self.readuntil.unblock_all {
            true => UnblockAll::from_str(&self.readuntil.unblock_all_mode).get_config(),
            false => (false, false, false, false)
        };

        // Easier access to the values in stream loops
        self.readuntil.unblock_all_client = unblock_all_client;
        self.readuntil.unblock_all_server = unblock_all_server;
        self.readuntil.unblock_all_basecaller = unblock_all_basecaller;
        self.readuntil.unblock_all_mapper = unblock_all_mapper;

        // Some checks and argument construction for basecaller configurations
        if self.dori.adaptive.classifier == Classifier::Minimap2Rust && self.experiment.reference.extension().expect("Could not extract extension of classifier reference path") != "mmi" {
            panic!("Minimap2 reference must be an index file ending with: .mmi")
        }
        // Some checks on file availability
        if self.dori.adaptive.classifier == Classifier::Minimap2Rust && !self.experiment.reference.exists() {
            panic!("Minimap2 reference does not exist at: {}", self.experiment.reference.display())
        }

        if self.dori.adaptive.basecaller == Basecaller::Guppy && (self.dori.adaptive.classifier == Classifier::Minimap2Rust  || self.dori.adaptive.classifier == Classifier::Kraken2) {
            
            self.basecaller.client.args = format!(
                "{} --address {} --config {} --throttle {} --max-reads-queued {} --threads {}",
                self.basecaller.client.script.display(),
                self.basecaller.client.address,
                self.basecaller.client.config,
                self.basecaller.client.throttle,
                self.basecaller.client.max_reads_queued,
                self.basecaller.client.threads,
            ).split_whitespace().map(String::from).collect();

            self.basecaller.server.args = format!(
                "--log_path {} --port {} --config {} --ipc_threads {} --device {} --gpu_runners_per_device {} --num_callers {} --chunks_per_runner {} --chunk_size {}",
                self.basecaller.server.log_path.display(),
                self.basecaller.server.port,
                self.basecaller.server.config,
                self.basecaller.server.threads,
                self.basecaller.server.device,
                self.basecaller.server.gpu_runners_per_device,
                self.basecaller.server.num_callers,
                self.basecaller.server.chunks_per_runner,
                self.basecaller.server.chunk_size,
            ).split_whitespace().map(String::from).collect();

        } else {
            panic!("Classifier configuration not supported")
        }

    }
    pub fn to_json(&self, file: &PathBuf) {
        serde_json::to_writer(
            &std::fs::File::create(&file).expect("Faile to create Streamfish configuration file"), &self
        ).expect("Failed to write Streamfish configuration to file")
    }
}

impl ExperimentConfig {
    pub fn configure(&mut self) {

        let experiment = match self.mode.as_str() {
            "mapping" => {  
                match self.r#type.as_str() {
                    "host_depletion" => {
                        Experiment::MappingExperiment(
                            MappingExperiment::HostDepletion(MappingConfig::host_depletion(self.targets.clone(), self.min_match_len))
                        )
                    },
                    "targeted_sequencing" => {
                        Experiment::MappingExperiment(
                            MappingExperiment::TargetedSequencing(MappingConfig::targeted_sequencing(self.targets.clone(), self.min_match_len))
                        )
                    },
                    "unknown_sequences" => {
                        Experiment::MappingExperiment(
                            MappingExperiment::UnknownSequences(MappingConfig::unknown_sequences(self.min_match_len))
                        )
                    },
                    _ => unimplemented!("Experiment type not implemented for mapping mode")
                }
            },
            _ => unimplemented!("Experiment mode not implemented")
        };

        self.experiment = experiment;

    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Experiment {
    // Alignment based experiment
    MappingExperiment(MappingExperiment)
}
impl Experiment {
    pub fn get_mapping_config(&self) -> MappingConfig {
        match self {
            Experiment::MappingExperiment(MappingExperiment::HostDepletion(host_depletion_config)) => host_depletion_config.clone(),
            Experiment::MappingExperiment(MappingExperiment::TargetedSequencing(targeted_sequencing_config)) => targeted_sequencing_config.clone(),
            Experiment::MappingExperiment(MappingExperiment::UnknownSequences(unknown_config)) => unknown_config.clone()
        }
    }
}
impl Default for Experiment {
    fn default() -> Self {
        Experiment::MappingExperiment(MappingExperiment::HostDepletion(MappingConfig::host_depletion(vec![], 0)))
    }
}


// An enumeration of `MappingConfig` variants wrapping configured structs
// that constitute an experimental setup as in Readfish (Table 2) + 
// additional presets specific to Streamfish
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MappingExperiment {
    HostDepletion(MappingConfig),
    TargetedSequencing(MappingConfig),
    UnknownSequences(MappingConfig)
}
impl MappingExperiment {
    // Region of interest for alignment: known host genome [implemented]
    pub fn host_depletion(targets: Vec<Target>, min_match_len: i32) -> Self {
        MappingExperiment::HostDepletion(
            MappingConfig::host_depletion(targets, min_match_len)
        )
    }
    // Includes experiment variants with regions of interest for alignment:
    //
    //   - Tageted regions: known regions from one or more genomes [implemented]
    //   - Targeted coverage depth: all known genomes within the sample, tracked for coverage depth [not implemented]
    //   - Low abundance enrichment: all genomes within the sample that can be identified as well as those that cannot [not implemented]
    //
    pub fn targeted_sequencing(targets: Vec<Target>, min_match_len: i32) -> Self {
        MappingExperiment::TargetedSequencing(
            MappingConfig::targeted_sequencing(targets, min_match_len)
        )
    }
    // Mapping against comprehensive database and targeting anything unknown
    pub fn unknown_sequences(min_match_len: i32) -> Self {
        MappingExperiment::UnknownSequences(
            MappingConfig::unknown_sequences(min_match_len)
        )
    }
}

// A mapping SAM flag configuration for alignment with Dorado (minimap2)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum MappingFlags {
    Multi,
    Single,
    None
}
impl MappingFlags {
    pub fn sam(self) -> Vec<u32> {
        match self {
            Self::Multi => vec![256, 257, 272],
            Self::Single => vec![0, 1, 16],
            Self::None => vec![4]
        }
    }
}

// Decision configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DecisionConfig {
    // We use the enum values here during instantiation 
    // because calling .into() methods repeatedly
    // introduces latency
    pub decision: i32, 
    pub flags: Vec<u32>
}


// A mapping configuration for alignment as outlined in Readfish (Tables 1)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MappingConfig {
    // List of target sequence ids
    pub targets: Vec<Target>,
    // Target all sequences in reference - used when target list is empty
    pub target_all: bool,
    // Read fragment maps multiple locations including region of interest.
    pub multi_on: DecisionConfig,
    // Read fragment maps to multiple locations not including region of interest.
    pub multi_off: DecisionConfig,
    // Read fragment only maps to region of interest.
    pub single_on: DecisionConfig,
    // Read fragment maps to one location but it is not a region of interest.
    pub single_off: DecisionConfig,
    // Read fragment does not map to the reference.
    pub no_map: DecisionConfig,
    // No sequence was obtained for the signal fragment.
    pub no_seq: DecisionConfig,
    // Minimum matched mapping length when using `minimap2-rust`
    pub min_match_len: i32,
}
impl MappingConfig {
    pub fn host_depletion(targets: Vec<Target>, min_match_len: i32) -> Self {
        Self {
            targets: targets.clone(),
            target_all: targets.is_empty(),
            min_match_len,

            multi_on: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::Multi.sam() 
            }, 
            multi_off: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::Multi.sam() 
            }, 
            single_on: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::Single.sam() 
            }, 
            single_off: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::Single.sam() 
            }, 
            no_map: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::None.sam() 
            },
            no_seq: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::None.sam() 
            }

        }
    }
    pub fn targeted_sequencing(targets: Vec<Target>, min_match_len: i32) -> Self {
        Self {
            targets: targets.clone(),
            target_all: targets.is_empty(),
            min_match_len,

            multi_on: DecisionConfig { 
                decision: Decision::StopData.into(), 
                flags: MappingFlags::Multi.sam() 
            }, 
            multi_off: DecisionConfig { 
                decision: Decision::Unblock.into(), // ES - 20240303 changed from Proceed
                flags: MappingFlags::Multi.sam() 
            }, 
            single_on: DecisionConfig { 
                decision: Decision::StopData.into(), 
                flags: MappingFlags::Single.sam() 
            }, 
            single_off: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::Single.sam() 
            }, 
            no_map: DecisionConfig { 
                decision: Decision::Proceed.into(),
                flags: MappingFlags::None.sam() 
            },
            no_seq: DecisionConfig { 
                decision: Decision::Proceed.into(),
                flags: MappingFlags::None.sam() 
            }
        }
    }
    pub fn unknown_sequences(min_match_len: i32) -> Self {
        Self {
            targets: Vec::new(),
            target_all: true,  // all mapped
            min_match_len,

            multi_on: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::Multi.sam() 
            }, 
            multi_off: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::Multi.sam() 
            }, 
            single_on: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::Single.sam() 
            }, 
            single_off: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::Single.sam() 
            }, 
            no_map: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::None.sam() 
            },
            no_seq: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::None.sam() 
            }
        }
    }
    #[deprecated(since = "0.1.0", note = "Dorado fork with streaming implementation has been removed due to instability and imminent release of server version.")]
    pub fn decision_from_sam(&self, flag: &u32, tid: &str) -> i32 {

        let target_mapped = match self.target_all {
            true => true,
            false => self.targets.iter().any(|x| x.reference == tid)
        };

        if self.multi_on.flags.contains(flag) && target_mapped {
            self.multi_on.decision
        } else if self.multi_off.flags.contains(flag) && !target_mapped {  
            self.multi_off.decision
        } else if self.single_on.flags.contains(flag) && target_mapped {
            self.single_on.decision
        } else if self.single_off.flags.contains(flag) && !target_mapped { 
            self.single_off.decision
        } else {
            self.no_map.decision
        }
    }
    // Main method for `minimap2-rs` using the Mapping struct to get a configured experiment decision
    pub fn decision_from_mapping(&self, mappings: Vec<Mapping>) -> i32 {

        let mappings: Vec<Mapping> = match self.min_match_len {
            0 => mappings,
            _ => {
                mappings.into_iter().filter(|x| {
                    x.match_len >= self.min_match_len
                }).collect()
            }
        };

        if mappings.is_empty() {
            return self.no_map.decision
        }

        let num_mappings = mappings.len();
        
        let target_mapped = match self.target_all {
            true => true,
            false => {
                let mut mapped = 0;
                for mapping in mappings.into_iter() {
                    if let Some(tid) = mapping.target_name {
                        // For each mapping test if it matches the aligned 
                        // sequence identifier and optionally if the alignment
                        // start falls within the target range
                        for target in &self.targets {
                            // CHANGED TO STARTSWITH HERE - ES 20240303
                            if target.reference.starts_with(&tid) { 
                                // If a target range is specified, test if the start OR end of the alignment
                                // falls within the target range - if so, this counts as a mapped read
                                if let (Some(start), Some(end)) = (target.start, target.end) {
                                    //  OR mapping end falls within range OR mapping spans the range
                                    if (mapping.target_start >= start && mapping.target_start <= end) ||  // alignment start falls within target range
                                       (mapping.target_end >= start && mapping.target_end <= end)     ||  // alignment end falls within target range
                                       (mapping.target_start <= start && mapping.target_end >= end)       // alignment spands the target range 
                                    {   
                                        mapped += 1
                                    }
                                } else {
                                    mapped += 1
                                }
                            }
                        }
                    }
                }
                mapped > 0
            }
        };
             
        if num_mappings > 1 && target_mapped {
            self.multi_on.decision
        } else if num_mappings > 1 && !target_mapped {
            self.multi_off.decision
        } else if num_mappings == 1 && target_mapped {
            self.single_on.decision
        } else if num_mappings == 1 && !target_mapped {
            self.single_off.decision 
        } else {
            self.no_map.decision
        }
    }
}



// Slice-and-dice configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SliceDiceConfig {
    pub channels: u32, 
    pub launch_dori_server: bool,
    pub launch_basecall_server: bool,
    pub slice: Vec<SliceConfig>
}
impl SliceDiceConfig {
    pub fn from_toml(file: &PathBuf) -> Result<Self, StreamfishConfigError> {

        let toml_str = std::fs::read_to_string(file).map_err(|err| StreamfishConfigError::TomlConfigFile(err))?;
        let config: SliceDiceConfig = toml::from_str(&toml_str).map_err(|err| StreamfishConfigError::TomlConfigParse(err))?;

        Ok(config)
    }
    // Get the re-configured core configs for each slice
    pub fn get_configs(&self, config: &StreamfishConfig) -> Vec<StreamfishConfig> {

        let mut configs = Vec::new();
        for slice in &self.slice {

            let mut slice_config = config.clone();

            slice_config.meta.client_name = slice.client_name.clone();
            slice_config.readuntil.launch_dori_server = self.launch_dori_server;
            slice_config.readuntil.launch_basecall_server = self.launch_basecall_server;

            slice_config.readuntil.channels = self.channels;
            slice_config.readuntil.channel_start = slice.channel_start;
            slice_config.readuntil.channel_end = slice.channel_end;
            slice_config.dori.adaptive.uds_path = slice.dori_adaptive_uds_path.clone();
            slice_config.basecaller.server.port = slice.basecaller_server_port.clone();
            slice_config.basecaller.client.address = slice.basecaller_client_address.clone();

            slice_config.configure(); // re-configure the basecaller server/client args

            configs.push(slice_config);
        }
        configs
    }
}

impl std::fmt::Display for SliceDiceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "SliceDice: channels={} slices={} launch_dori={} launch_basecaller={}", self.channels, self.slice.len(), self.launch_dori_server, self.launch_basecall_server)
    }
}

// Slice configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SliceConfig {
    pub client_name: String,
    pub channel_start: u32,
    pub channel_end: u32,
    pub dori_adaptive_uds_path: PathBuf,
    pub basecaller_server_port: String,
    pub basecaller_client_address: String,
}


impl std::fmt::Display for SliceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: start={} end={} dori={} basecaller_client={}", self.client_name, self.channel_start, self.channel_end, self.dori_adaptive_uds_path.display(), self.basecaller_client_address)
    }
}
