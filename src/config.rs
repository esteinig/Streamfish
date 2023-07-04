


use indoc::formatdoc;
use std::path::PathBuf;
use clap::crate_version;

use crate::services::minknow_api::data::get_live_reads_request::RawDataType;

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
    pub unblock_all: bool, 
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
                // Unblock-all latency tests
                unblock_all: false,                       // send unblock-all immediately after receipt
                unblock_all_dori: false,                 // send unblock-all through Dori but not analysis stack
                unblock_all_process: true,              // send unblock-all through configured processing stack on Dori
                // Is this relevant to latency?
                unblock_duration: 0.1,
                // Signal data configuration
                sample_minimum_chunk_size: 200,
                raw_data_type: RawDataType::Uncalibrated,
                accepted_first_chunk_classifications: vec![83, 65],
                // May need to increase these for larger pore arrays
                action_stream_queue_buffer: 2048,
                dori_stream_queue_buffer: 2048,
                logging_queue_buffer: 4096,
                // Logging configuration
                log_latency: None,
                print_latency: false,
                // Use streaming read cache for raw data chunks on Dori
                read_cache: true,
                read_cache_min_chunks: 0,
                read_cache_max_chunks: 3
            },
            dori: DoriConfig {
                uds_path: "/tmp/.dori/test".into(),
                uds_path_override: true,
                basecaller: "dorado".into(),
                basecaller_model_path: "/tmp/models/dna_r9.4.1_e8_fast@v3.4".into(),
                classifier: "minimap2".into(),
                classifier_reference_path: "/tmp/virosaurus.mmi".into(),
                basecaller_path: "/opt/dorado/bin/dorado".into(),
                classifier_path: "".into(),
                stderr_log: "/tmp/dori.pipeline.stderr".into(),
                kraken2_threads: 4,
                kraken2_args: "--minimum-hit-groups 1 --threads 4 --memory-mapping".into(),
                basecaller_args: Vec::new(),
                classifier_args: Vec::new()

            }
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
                streamfish_config.dori.basecaller_args = Vec::from([
                    "basecaller".to_string(),
                    "--reference".to_string(), 
                    format!("{}", streamfish_config.dori.classifier_reference_path.display()), 
                    "--emit-sam".to_string(),
                    format!("{}", streamfish_config.dori.basecaller_model_path.display()), 
                    "-".to_string()
                ]);       
            } else if streamfish_config.dori.basecaller == "dorado" && streamfish_config.dori.classifier == "kraken2" {
                // Basecalling only configuration with stdout pipe from Dorado
                streamfish_config.dori.basecaller_args = Vec::from([
                    "basecaller".to_string(),
                    "--emit-fastq".to_string(),
                    format!("{}", streamfish_config.dori.basecaller_model_path.display()), 
                    "-".to_string()
                ]);  

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