


use indoc::formatdoc;
use std::path::PathBuf;
use clap::crate_version;

use crate::services::{minknow_api::data::get_live_reads_request::RawDataType, dori_api::adaptive::Decision};

fn get_env_var(var: &str) -> String {
    std::env::var(var).expect(&format!("Failed to load environmental variable: {}", var))
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
    pub tls_cert_path: PathBuf,
}

// A run configuration for ReadUntilClient::run - some can be configured on command-line execution
#[derive(Debug, Clone)]
pub struct ReadUntilConfig {
    pub device_name: String,                 // Dori access to Minknow
    pub channel_start: u32,
    pub channel_end: u32,
    pub init_delay: u64,
    pub unblock_all_client: bool, 
    pub unblock_all_dori: bool,
    pub unblock_all_process: bool,
    pub unblock_duration: f64, 
    pub raw_data_type: RawDataType,
    pub sample_minimum_chunk_size: u64,
    pub accepted_first_chunk_classifications: Vec<i32>,
    pub action_stream_queue_buffer: usize,
    pub dori_stream_queue_buffer: usize,
    pub logging_queue_buffer: usize,
    pub log_latency: Option<PathBuf>,           // log latency output to file - adds latency! (1-2 bp)
    pub print_latency: bool,                    // if no latency file specified, print latency to console, otherwise standard log is used without latency
    pub read_cache: bool,
    pub read_cache_batch_rpc: bool,
    pub read_cache_min_chunks: usize,
    pub read_cache_max_chunks: usize
}

#[derive(Debug, Clone)]
pub struct DoriConfig {
    // Unix domain socket path
    pub uds_path: PathBuf,
    pub uds_path_override: bool,
    
    // Basecaller supported: `dorado`
    pub basecaller: String, 
    // Basecaller model path
    pub basecaller_model_path: PathBuf,
    // Classifier supported: `minimap2`, `kraken2`
    pub classifier: String,   
    // Classifier reference path
    pub classifier_reference_path: PathBuf,

    // Executable paths
    pub basecaller_path: PathBuf,
    pub classifier_path: PathBuf,

    // Basecaller/classifier error log
    pub stderr_log: PathBuf,

    // Dorado config
    pub dorado_batch_size: u32,
    pub dorado_mm_kmer_size: u32,
    pub dorado_mm_window_size: u32,

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
    // ReadUntil client configuration
    pub readuntil: ReadUntilConfig,
    // Adaptive sampling experiment configuration
    pub experiment: ExperimentConfig
}

impl StreamfishConfig {
    pub fn new(dot_env: bool) -> StreamfishConfig {

        if dot_env {
            dotenvy::dotenv().expect("Could not find '.env' file in directory tree");
        }
        
        let mut streamfish_config = Self {
            version: crate_version!().to_string(),
            minknow: MinKnowConfig {
                host: get_env_var("STREAMFISH_MINKNOW_HOST"),
                port: get_env_var("STREAMFISH_MINKNOW_PORT").parse::<i32>().unwrap(),
                token: get_env_var("STREAMFISH_MINKNOW_TOKEN"),
                tls_cert_path: get_env_var("STREAMFISH_MINKNOW_TLS_CERT_PATH").into(),
            },
            readuntil: ReadUntilConfig {
                // Device and target pore configuration
                device_name: "MS12345".to_string(),
                channel_start: 1,
                channel_end: 512,
                // Initiation of streams, delay to let 
                // analysis pipeline load models / indices
                init_delay: 10,
                // Unblock-all latency tests
                unblock_all_client: false,               // send unblock-all immediately after receipt
                unblock_all_dori: true,                  // send unblock-all through Dori but not analysis stack
                unblock_all_process: false,              // send unblock-all through configured processing stack on Dori
                // Is this relevant to latency?
                unblock_duration: 0.1,
                // Signal data configuration
                sample_minimum_chunk_size: 200,
                raw_data_type: RawDataType::Uncalibrated,
                accepted_first_chunk_classifications: vec![83, 65],
                // May need to increase these for larger pore arrays
                // and for stream stability - looks like in the cached
                // endpoints with multiple responses we might cause
                // instability
                action_stream_queue_buffer: 10000,
                dori_stream_queue_buffer: 20000,
                logging_queue_buffer: 10000,
                // Logging configuration
                log_latency: None,
                print_latency: false,
                // Use streaming read cache for raw data chunks on Dori
                read_cache: true,
                // Send data as batched request to Dori cache RPC, otherwise single channel requests
                read_cache_batch_rpc: false,
                // There is some limit here where I think the UDS connection
                // becomes overloaded and breaks Need to test two solutions:
                // fist it maybe due to UDS instability so replace with TCP
                // second, send sampled channel chunks in one go to Dori,
                // instead of for each channel [unblock all process]
                //
                // I think it's mainly the by-channel implementation that can
                // overload the UDS connection - empirically it seems to happen
                // when we process by single chunks and send individually by
                // channel... but not always, maybe some instability expected.
                // 
                // Still make sure to test TCP
                read_cache_min_chunks: 1,
                read_cache_max_chunks: 1
            },
            dori: DoriConfig {
                uds_path: "/tmp/.dori/test".into(),
                uds_path_override: true,
                basecaller: "dorado".into(),
                basecaller_model_path: "/tmp/models/dna_r9.4.1_e8_fast@v3.4".into(),
                classifier: "minimap2".into(),
                classifier_reference_path: "/tmp/virosaurus.mmi".into(),
                basecaller_path: "/home/esteinig/dev/bin/dorado".into(),
                classifier_path: "".into(),
                stderr_log: "/tmp/dori.pipeline.stderr".into(),
                // There are some diminishing returns for batch size reduction - it probably 
                // depend on the number of pores sequencing at the time - too small a batch 
                // size and latency increases because the stream is pushing too many packets
                // too big a batch size and latency increases because basecaller is waiting 
                // for more packages - not sure how to optimize, but a batch-size = 32 works
                // better on the test data than batch-size = 64 or 16 - and works better in 
                // batched transmission cache mode?
                dorado_batch_size: 64,
                dorado_mm_kmer_size: 15,
                dorado_mm_window_size: 10,
                kraken2_threads: 4,
                kraken2_args: "--minimum-hit-groups 1 --threads 4 --memory-mapping".into(),
                basecaller_args: Vec::new(),
                classifier_args: Vec::new()
            },
            // Default host depletion experiment - change or read from file!
            experiment: ExperimentConfig::default()
        };

        // Some checks and argument construction for basecaller/classifier configurations

        if !["dorado"].contains(&streamfish_config.dori.basecaller.trim().to_lowercase().as_str()) {
            panic!("Basecaller configuration not supported")
        }
        if !["minimap2", "kraken2"].contains(&streamfish_config.dori.classifier.trim().to_lowercase().as_str()) {
            panic!("Classifier configuration not supported")
        }

        if streamfish_config.dori.classifier == "minimap2" && streamfish_config.dori.classifier_reference_path.extension().expect("Could not extract extension of classifier reference path") != "mmi" {
            panic!("Classifier reference for minimap2 must be an index file (.mmi)")
        }

        // Dorado basecaller setup
        if streamfish_config.dori.basecaller_args.is_empty() {
            // Construct arguments and set into config
            if streamfish_config.dori.basecaller == "dorado"  && streamfish_config.dori.classifier == "minimap2" {
                // Minimap2 configuration integrated with Dorado - add window/k-mer options [TODO]
                streamfish_config.dori.basecaller_args = format!(
                    "basecaller --verbose --batchsize {} --reference {} -k {} -w {} -g 8 --emit-sam {} -",
                    streamfish_config.dori.dorado_batch_size,
                    streamfish_config.dori.classifier_reference_path.display(),
                    streamfish_config.dori.dorado_mm_kmer_size,
                    streamfish_config.dori.dorado_mm_window_size,
                    streamfish_config.dori.basecaller_model_path.display()
                ).split_whitespace().map(String::from).collect();  
            } else if streamfish_config.dori.basecaller == "dorado" && streamfish_config.dori.classifier == "kraken2" {
                // Basecalling only configuration with stdout pipe from Dorado
                streamfish_config.dori.basecaller_args = format!(
                    "basecaller --verbose --batchsize {} --emit-fastq {} -",
                    streamfish_config.dori.dorado_batch_size,
                    streamfish_config.dori.basecaller_model_path.display()
                ).split_whitespace().map(String::from).collect();
            } else {
                panic!("Classifier configuration not supported")
            }
        }


        if streamfish_config.dori.classifier_args.is_empty() {
            // Construct arguments and set into config
            if  streamfish_config.dori.basecaller == "dorado" && streamfish_config.dori.classifier == "kraken2" {
                for arg in  streamfish_config.dori.kraken2_args.clone().split_whitespace() {
                    streamfish_config.dori.classifier_args.push(arg.to_string())
                }
                streamfish_config.dori.classifier_args = Vec::from([
                    "--db".to_string(), 
                    format!("{}", streamfish_config.dori.classifier_reference_path.display()), 
                    "--threads".to_string(), 
                    streamfish_config.dori.kraken2_threads.to_string(),
                    "/dev/fd/0".to_string() // stdin
                ]);

            } else if streamfish_config.dori.basecaller == "dorado" && streamfish_config.dori.classifier == "minimap2" {
                // No settings necessary - minimap is used in Dorado
            } else {
                panic!("Classifier / basecaller combination not supported")
            }

        }

        return streamfish_config

    }
}

#[derive(Debug, Clone)]
pub enum Experiment {
    // Alignment based experiment
    MappingExperiment(MappingExperiment)
}



// An enumeration of `MappingConfig` variants wrapping configured structs
// that constitute an experimental setup as in Readfish (Table 2)
#[derive(Debug, Clone)]
pub enum MappingExperiment {
    HostDepletion(MappingConfig),
    TargetedSequencing(MappingConfig)
}
impl MappingExperiment {
    // Region of interest for alignment: known host genome [implemented]
    pub fn host_depletion() -> Self {
        MappingExperiment::HostDepletion(
            MappingConfig::host_depletion()
        )
    }
    // Includes experiment variants with regions of interes for alignment:
    //
    //   - Tageted regions: known regions from one or more genomes [implemented]
    //   - Targeted coverage depth: all known genomes within the sample, tracked for coverage depth [not implemented]
    //   - Low abundance enrichment: all genomes within the sample that can be identified as well as those that cannot [not implemented]
    //
    pub fn targeted_sequencing() -> Self {
        MappingExperiment::TargetedSequencing(
            MappingConfig::targeted_sequencing()
        )
    }
}

// A mapping SAM flag configuration for alignment with Dorado (minimap2)
#[derive(Debug, Clone)]
pub enum MappingFlags {
    MultiOn,
    MultiOff,
    SingleOn,
    SingleOff,
    NoMap,
    NoSeq
}
impl MappingFlags {
    pub fn sam(self) -> Vec<u32> {
        match self {
            Self::MultiOn => vec![],
            Self::MultiOff => vec![],
            Self::SingleOn => vec![],
            Self::SingleOff => vec![],
            Self::NoMap => vec![],
            Self::NoSeq => vec![],
        }
    }
}

// Decision configuration
#[derive(Debug, Clone)]
pub struct DecisionConfig {
    // We use the enum values here during instantiation 
    // because calling .into() methods repeatedly
    // introduces latency
    pub decision: i32, 
    pub flags: Vec<u32>
}

// A mapping configuration for alignment as outlined in Readfish (Tables 1)
#[derive(Debug, Clone)]
pub struct MappingConfig {
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
    pub fn host_depletion() -> Self {
        Self {
            multi_on: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::MultiOn.sam() 
            }, 
            multi_off: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::MultiOff.sam() 
            }, 
            single_on:  DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::SingleOn.sam() 
            }, 
            single_off:  DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::SingleOff.sam() 
            }, 
            no_map: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::NoMap.sam() 
            },
            no_seq: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::NoSeq.sam() 
            },
            unblock_all: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: vec![4, 0, 10]  // NOT USED - tests latency for unblock-all
            },
        }
    }
    pub fn targeted_sequencing() -> Self {
        Self {
            multi_on: DecisionConfig { 
                decision: Decision::StopData.into(), 
                flags: MappingFlags::MultiOn.sam() 
            }, 
            multi_off: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::MultiOff.sam() 
            }, 
            single_on:  DecisionConfig { 
                decision: Decision::StopData.into(), 
                flags: MappingFlags::SingleOn.sam() 
            }, 
            single_off:  DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: MappingFlags::SingleOff.sam() 
            }, 
            no_map: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::NoMap.sam() 
            },
            no_seq: DecisionConfig { 
                decision: Decision::Proceed.into(), 
                flags: MappingFlags::NoSeq.sam() 
            },
            unblock_all: DecisionConfig { 
                decision: Decision::Unblock.into(), 
                flags: vec![4, 0, 10]  // NOT USED - tests latency for unblock-all
            }
        }
    }
    // Main method to pass a flag and get the configured experiment decision
    pub fn decision(&self, flag: &u32) -> i32 {
        if self.multi_on.flags.contains(flag) {
            self.multi_on.decision
        } else if self.multi_off.flags.contains(flag) {
            self.multi_off.decision
        } else if self.single_on.flags.contains(flag) {
            self.single_on.decision
        } else if self.single_off.flags.contains(flag) {
            self.single_off.decision
        } else {
            // No sequence is not possible, so we otherwise 
            // return no mapping status decision
            self.no_map.decision
        }
    }
    // A test method that returns an unblock decision for unblock-all testing
    pub fn unblock_all(&self, flag: &u32) -> i32 {
        if self.unblock_all.flags.contains(flag) {
            // Not action, always return Unblock for testing
        }
        self.unblock_all.decision
    }
}




// A configuration for the experimental setup that should be dynamically
// editable through linking a slow analysis loop into the stream loop 
// (not quite sure how to do this yet) 
#[derive(Debug, Clone)]
pub struct ExperimentConfig {
    pub name: String,
    pub version: String,
    pub description: String,
    pub config: Experiment
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            name: String::from("Default"),
            version: String::from("v0.1.0"),
            description: String::from("Default adaptive sampling configuration uses host genome depletion with alignment"),
            config: Experiment::MappingExperiment(
                MappingExperiment::HostDepletion(MappingConfig::host_depletion())
            )
        }
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