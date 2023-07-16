


use indoc::formatdoc;
use minimap2::Mapping;
use std::path::PathBuf;
use clap::crate_version;
use serde::Deserialize;

use crate::services::{minknow_api::data::get_live_reads_request::RawDataType, dori_api::adaptive::Decision};

fn get_env_var(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

// An exposed subset of configurable parameters for the user
#[derive(Debug, Clone, Deserialize)]
pub struct UserConfig  {
    pub meta: UserConfigMetadata,
    pub minknow: UserConfigMinknow,
    pub icarust: UserConfigIcarust,
    pub dorado: UserConfigDorado,
    pub dori: UserConfigDori,
    pub readuntil: UserConfigReadUntil,
    pub experiment: UserConfigExperiment
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigMetadata {
    pub name: String,
    pub version: String,
    pub description: String
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigMinknow {
    pub port: i32,
    pub host: String,
    pub token: String,
    pub certificate: PathBuf,
}


#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigIcarust {
    pub enabled: bool,
    pub position_port: u32,
    pub sample_rate: u32
}


#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigDori {
    pub tcp_enabled: bool,
    pub tcp_port: u32,
    pub tcp_host: String,           // inside docker to expose must be 0.0.0.0
    pub uds_path: PathBuf,
    pub uds_override: bool,
    pub minknow_host: String,
    pub minknow_port: u32,
    pub classifier: String,
    pub basecaller_path: PathBuf,
    pub basecaller_model: PathBuf,
    pub basecaller_stderr: PathBuf
}


#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigReadUntil {
    pub init_delay: u64,
    pub device_name: String,              
    pub channel_start: u32,
    pub channel_end: u32,
    pub dori_tcp_host: String,  // can be different if outside of container
    pub dori_tcp_port: u32,    
    pub unblock_all: bool,
    pub unblock_all_mode: String,
    pub read_cache: bool,
    pub read_cache_batch_rpc: bool,
    pub read_cache_min_chunks: usize,
    pub read_cache_max_chunks: usize,
    pub throttle: u64,
    pub latency_log: Option<PathBuf>,

    pub unblock_duration: f64,
    pub sample_minimum_chunk_size: u64,
    pub accepted_first_chunk_classifications: Vec<i32>
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigDorado{
    pub batch_size: u32,
    pub batch_timeout: u32,
    pub model_runners: u32,
    pub minimap_kmer_size: u32,
    pub minimap_window_size: u32,
    pub minimap_index_batch_size: String
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserConfigExperiment {
    pub control: bool,
    pub mode: String,
    pub r#type: String,
    pub reference: PathBuf,
    pub targets: Vec<String>
}

/// Unblock all circuits for testing
#[derive(Debug, Clone, PartialEq)]
pub enum UnblockAll {
    Client,
    Server,
    Process
}
impl UnblockAll {
    fn from_str(s: &str) -> Self {
        match s {
            "client" => Self::Client,
            "server" => Self::Server,
            "process" => Self::Process,
            _ => unimplemented!("Unblock all setting `{}` is not implemented", s)
        }
    }
    // A little clunky but betetr to have bools in configuration for 
    // evaluation in streams than matching enumeration types or values
    fn get_config(&self) -> (bool, bool, bool) {
        match self {
            UnblockAll::Client => return (true, false, false),
            UnblockAll::Server => return (false, true, false),
            UnblockAll::Process => return (false, false, true)
        }
    }
}

/// Basecallers
#[derive(Debug, Clone, PartialEq)]
pub enum Basecaller {
    Dorado
}
impl Basecaller {
    pub fn from_str(s: &str) -> Self {
        match s {
            "dorado" => Self::Dorado,
            _ => unimplemented!("Basecaller `{}` is not implemented", s)
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Basecaller::Dorado => "dorado"
        }
    }
}

/// Classifiers
#[derive(Debug, Clone, PartialEq)]
pub enum Classifier {
    Minimap2Dorado,
    Minimap2Rust,
    Kraken2
}
impl Classifier {
    pub fn from_str(s: &str) -> Self {
        match s {
            "minimap2-dorado" => Self::Minimap2Dorado,
            "minimap2-rs" => Self::Minimap2Rust,
            "kraken2" => Self::Kraken2,
            _ => unimplemented!("Classifier `{}` is not implemented", s)
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Classifier::Minimap2Dorado => "minimap2-dorado",
            Classifier::Minimap2Rust => "minimap2-rs",
            Classifier::Kraken2 => "kraken2"
        }
    }
}

#[derive(Debug, Clone)]
pub struct MinKnowConfig {
    // Host address that runs MinKnow
    pub host: String,
    // Port of MinKnow manager service [9502]
    pub port: i32,
    // Developer token generated in MinKnow UI
    pub token: String,
    // TLS certificate path, required to connect to MinKnow API
    pub tls_cert_path: PathBuf
}

// Uses the MinKnow connection but adds required additional parameters
#[derive(Debug, Clone)]
pub struct IcarustConfig {
    // Whether the data generation is simulated by Icarust
    pub enabled: bool,
    // Port of the device connection - usually sourced 
    // from MinKNOW after connection, needs to be
    // specified with Icarust
    pub position_port: u32,
    // Icarust does not expose the sample rate from device endpoint
    // so we must set this manually for now (defualt 4000)
    pub sample_rate: u32,
}


// ReadUntil client configuration
#[derive(Debug, Clone)]
pub struct ReadUntilConfig {
    pub device_name: String,                 // Dori access to Minknow
    pub channel_start: u32,
    pub channel_end: u32,
    pub dori_tcp_host: String,
    pub dori_tcp_port: u32,
    pub init_delay: u64,                     // u64 because of duration type
    pub unblock_all_client: bool,
    pub unblock_all_server: bool,
    pub unblock_all_process: bool,
    pub unblock_duration: f64, 
    pub raw_data_type: RawDataType,
    pub sample_minimum_chunk_size: u64,
    pub accepted_first_chunk_classifications: Vec<i32>,
    pub throttle: u64,
    pub latency_log: Option<PathBuf>,          
    pub read_cache: bool,
    pub read_cache_batch_rpc: bool,
    pub read_cache_min_chunks: usize,
    pub read_cache_max_chunks: usize
}

// Dori RPC server configuration
#[derive(Debug, Clone)]
pub struct DoriConfig {
    // TCP server connection  
    pub tcp_enabled: bool,
    pub tcp_port: u32,
    pub tcp_host: String,

    // Unix domain socket connection
    pub uds_path: PathBuf,
    pub uds_path_override: bool,

    // MinKNOW / Icarust connection from Dori 
    // added for host/container setups
    pub minknow_host: String,
    pub minknow_port: u32,
    
    // Basecaller supported: `dorado`
    pub basecaller: Basecaller, 
    // Basecaller model path
    pub basecaller_model_path: PathBuf,
    // Classifier supported: `minimap2`, `kraken2`
    pub classifier: Classifier,   
    // Classifier reference path
    pub classifier_reference_path: PathBuf,

    // Executable paths
    pub basecaller_path: PathBuf,
    pub classifier_path: PathBuf,

    // Basecaller/classifier error log
    pub stderr_log: PathBuf,

    // Dorado config
    pub dorado_batch_size: u32,
    pub dorado_batch_timeout: u32,
    pub dorado_model_runners: u32,
    pub dorado_mm_kmer_size: u32,
    pub dorado_mm_window_size: u32,
    pub dorado_mm_index_batch_size: String,

    // Kraken2 config
    pub kraken2_threads: u16,
    pub kraken2_args: String,

    // Internal configs
    pub classifier_args: Vec<String>,
    pub basecaller_args: Vec<String>
}

#[derive(Debug, Clone)]
pub struct StreamfishConfig {
    // Streamfish version
    pub version: String,
    // Dori server configuration
    pub dori: DoriConfig,
    // MinKnow client configuration
    pub minknow: MinKnowConfig,
    // Icarust -> Minknow configuration
    pub icarust: IcarustConfig,
    // ReadUntil client configuration
    pub readuntil: ReadUntilConfig,
    // Adaptive sampling experiment configuration
    pub experiment: ExperimentConfig
}

impl StreamfishConfig {
    pub fn new(user_config: PathBuf) -> StreamfishConfig {

        let user_config_str = std::fs::read_to_string(user_config).expect("Failed to read user config TOML");
        let user_config: UserConfig = toml::from_str(&user_config_str).expect("Failed to load user config TOML");

        let (unblock_all_client, unblock_all_server, unblock_all_process) = match user_config.readuntil.unblock_all {
            true => UnblockAll::from_str(&user_config.readuntil.unblock_all_mode).get_config(),
            false => (false, false, false)
        };

        let mut streamfish_config = Self {
            version: crate_version!().to_string(),

            experiment: ExperimentConfig::from(
                user_config.clone()
            ),

            minknow: MinKnowConfig {
                host: match get_env_var("STREAMFISH_MINKNOW_HOST") { Some(var) => var, None => user_config.minknow.host.clone() },  // needed for Docker container forward host
                port: user_config.minknow.port.clone(),
                token: user_config.minknow.token.clone(),
                tls_cert_path: user_config.minknow.certificate.clone(),
            },
            icarust: IcarustConfig { enabled: user_config.icarust.enabled, position_port: user_config.icarust.position_port, sample_rate: user_config.icarust.sample_rate },

            readuntil: ReadUntilConfig {
                device_name: user_config.readuntil.device_name,
                channel_start: user_config.readuntil.channel_start,
                channel_end: user_config.readuntil.channel_end,
                dori_tcp_host: user_config.readuntil.dori_tcp_host,
                dori_tcp_port: user_config.readuntil.dori_tcp_port,
                init_delay: user_config.readuntil.init_delay,
                unblock_all_client: unblock_all_client,
                unblock_all_server: unblock_all_server,
                unblock_all_process: unblock_all_process,
                unblock_duration: user_config.readuntil.unblock_duration,
                throttle: user_config.readuntil.throttle,
                sample_minimum_chunk_size: user_config.readuntil.sample_minimum_chunk_size,
                raw_data_type: RawDataType::Uncalibrated,
                accepted_first_chunk_classifications: user_config.readuntil.accepted_first_chunk_classifications,
                latency_log: user_config.readuntil.latency_log,
                read_cache: user_config.readuntil.read_cache,
                read_cache_batch_rpc: user_config.readuntil.read_cache_batch_rpc,
                read_cache_min_chunks: user_config.readuntil.read_cache_min_chunks,
                read_cache_max_chunks: user_config.readuntil.read_cache_max_chunks
            },

            dori: DoriConfig {
                tcp_enabled: user_config.dori.tcp_enabled,
                tcp_port: user_config.dori.tcp_port,
                tcp_host: user_config.dori.tcp_host,  // inside docker to expose must be 0.0.0.0
                uds_path: user_config.dori.uds_path,
                uds_path_override: user_config.dori.uds_override,
                minknow_host: user_config.dori.minknow_host,
                minknow_port: user_config.dori.minknow_port,
                basecaller: Basecaller::Dorado,
                basecaller_model_path: user_config.dori.basecaller_model,
                classifier: Classifier::from_str(&user_config.dori.classifier),
                classifier_reference_path: user_config.experiment.reference,
                basecaller_path: user_config.dori.basecaller_path,  // /usr/src/streamfish/scripts/cpp/cmake-build/test | /home/esteinig/dev/bin/dorado | /opt/dorado/bin/dorado
                classifier_path: "".into(),
                stderr_log: user_config.dori.basecaller_stderr,
                dorado_model_runners: user_config.dorado.model_runners,
                dorado_batch_size: user_config.dorado.batch_size,
                dorado_batch_timeout: user_config.dorado.batch_timeout,
                dorado_mm_kmer_size: user_config.dorado.minimap_kmer_size,
                dorado_mm_window_size: user_config.dorado.minimap_window_size,
                dorado_mm_index_batch_size: user_config.dorado.minimap_index_batch_size,
                kraken2_threads: 4,
                kraken2_args: "--minimum-hit-groups 1 --threads 4 --memory-mapping".into(),

                basecaller_args: Vec::new(),
                classifier_args: Vec::new()
            }
        };

        // Some checks and argument construction for basecaller/classifier configurations

        if (streamfish_config.dori.classifier == Classifier::Minimap2Dorado || streamfish_config.dori.classifier == Classifier::Minimap2Rust) && streamfish_config.dori.classifier_reference_path.extension().expect("Could not extract extension of classifier reference path") != "mmi" {
            panic!("Classifier reference for minimap2 must be an index file (.mmi)")
        }

        // Dorado basecaller setup
        if streamfish_config.dori.basecaller_args.is_empty() {
            // Construct arguments and set into config
            if streamfish_config.dori.basecaller == Basecaller::Dorado  && streamfish_config.dori.classifier == Classifier::Minimap2Dorado {
                // Minimap2 configuration integrated with Dorado - add window/k-mer options [TODO]
                streamfish_config.dori.basecaller_args = format!(
                    "basecaller --verbose --batchsize {} --reference {} -I {} -k {} -w {} -g 64 --batch-timeout {} --num-runners {} --emit-sam {} -", // 
                    streamfish_config.dori.dorado_batch_size,
                    streamfish_config.dori.classifier_reference_path.display(),
                    streamfish_config.dori.dorado_mm_index_batch_size,
                    streamfish_config.dori.dorado_mm_kmer_size,
                    streamfish_config.dori.dorado_mm_window_size,
                    streamfish_config.dori.dorado_batch_timeout,
                    streamfish_config.dori.dorado_model_runners,
                    streamfish_config.dori.basecaller_model_path.display()
                ).split_whitespace().map(String::from).collect();  
            } else if streamfish_config.dori.basecaller == Basecaller::Dorado && (streamfish_config.dori.classifier == Classifier::Minimap2Rust  || streamfish_config.dori.classifier == Classifier::Kraken2) {
                // Basecalling only configuration with stdout pipe from Dorado
                streamfish_config.dori.basecaller_args = format!(
                    "basecaller --verbose --batchsize {} -g 64 --batch-timeout {} --num-runners {} --emit-fastq {} -",
                    streamfish_config.dori.dorado_batch_size,
                    streamfish_config.dori.dorado_batch_timeout,
                    streamfish_config.dori.dorado_model_runners,
                    streamfish_config.dori.basecaller_model_path.display()
                ).split_whitespace().map(String::from).collect();
            } else {
                panic!("Classifier configuration not supported")
            }
        }

        return streamfish_config

    }
    pub fn cli_config(&mut self, channel_start: Option<u32>, channel_end: Option<u32>, port_dori: Option<u32>, log_latency: Option<PathBuf>) {

        // Some general client configurations can be set from the command-line
        // and are overwritten before connection of the clients

        if let Some(log_file) = log_latency {
            self.readuntil.latency_log = Some(log_file)
        }
        if let Some(tcp_port) = port_dori {
            self.dori.tcp_port = tcp_port;
        }
        if let Some(start) = channel_start {
            self.readuntil.channel_start = start;
        }
        if let Some(end) = channel_end {
            self.readuntil.channel_end = end;
        }
    }
}



// A configuration for the experimental setup that should be dynamically
// editable through linking a slow analysis loop into the stream loop 
// (not quite sure how to do this yet) 
#[derive(Debug, Clone, Deserialize)]
pub struct ExperimentConfig {
    pub control: bool,
    pub name: String,
    pub version: String,
    pub description: String,
    pub config: Experiment
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            control: false,
            name: String::from("Default"),
            version: String::from("v0.1.0"),
            description: String::from("Default adaptive sampling configuration uses host genome depletion with alignment"),
            config: Experiment::MappingExperiment(
                MappingExperiment::HostDepletion(MappingConfig::host_depletion(Vec::new())) // whole reference genome
            )
        }
    }
}

impl ExperimentConfig {
    pub fn from(user_config: UserConfig) -> Self {

        let experiment = match user_config.experiment.mode.as_str() {
            "mapping" => {  
                match user_config.experiment.r#type.as_str() {
                    "host_depletion" => {
                        Experiment::MappingExperiment(
                            MappingExperiment::HostDepletion(MappingConfig::host_depletion(user_config.experiment.targets))
                        )
                    },
                    "targeted_sequencing" => {
                        Experiment::MappingExperiment(
                            MappingExperiment::TargetedSequencing(MappingConfig::targeted_sequencing(user_config.experiment.targets))
                        )
                    },
                    "unknown_sequences" => {
                        Experiment::MappingExperiment(
                            MappingExperiment::UnknownSequences(MappingConfig::unknown_sequences())
                        )
                    },
                    _ => unimplemented!("Experiment type not implemented for mapping mode")
                }
            },
            _ => unimplemented!("Experiment mode not implemented")
        };

        Self {
            control: user_config.experiment.control,
            name: user_config.meta.name,
            version: user_config.meta.version,
            description: user_config.meta.description,
            config: experiment
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub enum Experiment {
    // Alignment based experiment
    MappingExperiment(MappingExperiment)
}



// An enumeration of `MappingConfig` variants wrapping configured structs
// that constitute an experimental setup as in Readfish (Table 2) + 
// additional presets specific to Streamfish
#[derive(Debug, Clone, Deserialize)]
pub enum MappingExperiment {
    HostDepletion(MappingConfig),
    TargetedSequencing(MappingConfig),
    UnknownSequences(MappingConfig)
}
impl MappingExperiment {
    // Region of interest for alignment: known host genome [implemented]
    pub fn host_depletion(targets: Vec<String>) -> Self {
        MappingExperiment::HostDepletion(
            MappingConfig::host_depletion(targets)
        )
    }
    // Includes experiment variants with regions of interest for alignment:
    //
    //   - Tageted regions: known regions from one or more genomes [implemented]
    //   - Targeted coverage depth: all known genomes within the sample, tracked for coverage depth [not implemented]
    //   - Low abundance enrichment: all genomes within the sample that can be identified as well as those that cannot [not implemented]
    //
    pub fn targeted_sequencing(targets: Vec<String>) -> Self {
        MappingExperiment::TargetedSequencing(
            MappingConfig::targeted_sequencing(targets)
        )
    }
    // Mapping against comprehensive database and targeting anything unknown
    pub fn unknown_sequences() -> Self {
        MappingExperiment::UnknownSequences(
            MappingConfig::unknown_sequences()
        )
    }
}

// A mapping SAM flag configuration for alignment with Dorado (minimap2)
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct DecisionConfig {
    // We use the enum values here during instantiation 
    // because calling .into() methods repeatedly
    // introduces latency
    pub decision: i32, 
    pub flags: Vec<u32>
}

// A mapping configuration for alignment as outlined in Readfish (Tables 1)
#[derive(Debug, Clone, Deserialize)]
pub struct MappingConfig {
    // List of target sequence ids
    pub targets: Vec<String>,
    // Target all sequences in reference - used when target list is empty
    pub target_all: bool,
    //	Read fragment maps multiple locations including region of interest.
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
    // No sequence was obtained for the signal fragment.
    pub unblock_all: DecisionConfig
}
impl MappingConfig {
    pub fn host_depletion(targets: Vec<String>) -> Self {
        Self {
            targets: targets.clone(),
            target_all: targets.is_empty(),

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
            },
            unblock_all: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: vec![4, 0, 10]  // NOT USED - tests latency for unblock-all
            },
        }
    }
    pub fn targeted_sequencing(targets: Vec<String>) -> Self {
        Self {
            targets: targets.clone(),
            target_all: targets.is_empty(),

            multi_on: DecisionConfig { 
                decision: Decision::StopData.into(), 
                flags: MappingFlags::Multi.sam() 
            }, 
            multi_off: DecisionConfig { 
                decision: Decision::Proceed.into(), 
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
            },
            unblock_all: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: vec![4, 0, 10]  // NOT USED - tests latency for unblock-all
            }
        }
    }
    pub fn unknown_sequences() -> Self {
        Self {
            targets: Vec::new(),
            target_all: true,  // all mapped

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
            },
            unblock_all: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: vec![4, 0, 10]  // NOT USED - tests latency for unblock-all
            }
        }
    }
    // Main method for `minimap2-dorado` using a SAM flag to get the configured experiment decision [.contains() does not work with str slices]
    pub fn decision_from_sam(&self, flag: &u32, tid: &str) -> i32 {

        let target_mapped = match self.target_all {
            true => true,
            false => self.targets.iter().any(|x| x == tid)
        };

        if self.multi_on.flags.contains(flag) && target_mapped {
            self.multi_on.decision
        } else if self.multi_off.flags.contains(flag) && !target_mapped {  // target all sequences active (no sequences in targets) == any sequence mapping off target as targets do not exist
            self.multi_off.decision
        } else if self.single_on.flags.contains(flag) && target_mapped {
            self.single_on.decision
        } else if self.single_off.flags.contains(flag) && !target_mapped { // target all sequences active (no sequences in targets) == any sequence mapping off target as targets do not exist
            self.single_off.decision
        } else {
            // No sequence is not possible, so we otherwise 
            // return no mapping status decision
            self.no_map.decision
        }
    }
    // Main method for `minimap2-rs` using the Mapping struct to get a configured experiment decision
    pub fn decision_from_mapping(&self, mappings: Vec<Mapping>, unblock_all: bool, ) -> i32 {

        if unblock_all {
            return self.unblock_all.decision;
        }

        if mappings.is_empty() {
            return self.no_map.decision
        }

        let mappings: Vec<Mapping> = mappings.into_iter().filter(|x| x.match_len >= 100).collect();

        let num_mappings = mappings.len();
        
        let target_mapped = match self.target_all {
            true => true,
            false => {
                let mut mapped = Vec::new();
                for mapping in mappings.into_iter() {
                    if let Some(tid) = mapping.target_name {
                        log::info!("Detected mapping for {}", tid);
                        mapped.push(tid);
                    }
                }
                self.targets.iter().any(|target| mapped.contains(&target)) 
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
    // A test method that returns an unblock decision for unblock-all testing
    pub fn unblock_all(&self, flag: &u32, tid: &str) -> i32 {
        if self.unblock_all.flags.contains(flag) && (self.target_all || self.targets.iter().any(|x| x == tid)) {
            // No action, always return unblock for testing, implements one lo logic check to reflect decision logic time
        }
        self.unblock_all.decision
    }
}


impl std::fmt::Display for StreamfishConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = formatdoc! {"

        ========================
        Streamfish configuration
        ========================

        MinKnow Host    {minknow_host}
        MinKnow Port    {minknow_port}
        MinKnow Token   {minknow_token}

        ",
        minknow_host = self.minknow.host,
        minknow_port = self.minknow.port,
        minknow_token = self.minknow.token,
    };
        
        write!(f, "{}", s)
    }
}