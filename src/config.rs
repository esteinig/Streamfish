


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
    pub device_name: String,  // allows server to have access to Minknow
    pub channel_start: u32,
    pub channel_end: u32,
    pub unblock_all: bool, 
    pub unblock_dori: bool,
    pub unblock_duration: f64, 
    pub raw_data_type: RawDataType,
    pub sample_minimum_chunk_size: u64,
    pub accepted_first_chunk_classifications: Vec<i32>,
    pub action_stream_queue_buffer: usize,
    pub dori_stream_queue_buffer: usize,
    pub logging_queue_buffer: usize,
}

#[derive(Debug, Clone)]
pub struct DoriConfig {
    // Unix domain socket path
    // implemenmt TCP later in case of 
    // serving remotely
    pub uds_path: PathBuf,
    pub uds_path_override: bool,
    pub dorado_path: PathBuf,
    pub dorado_args: String,
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
        
        Self {
            version: crate_version!().to_string(),
            minknow: MinKnowConfig {
                host: get_env_var("STREAMFISH_MINKNOW_HOST"),
                port: get_env_var("STREAMFISH_MINKNOW_PORT").parse::<i32>().unwrap(),
                token: get_env_var("STREAMFISH_MINKNOW_TOKEN"),
                tls_cert_path: get_env_var("STREAMFISH_MINKNOW_TLS_CERT_PATH").into(),
            },
            readuntil: ReadUntilConfig {
                device_name: "MS12345".to_string(),
                channel_start: 1,
                channel_end: 512,
                unblock_all: false,
                unblock_dori: false,
                unblock_duration: 0.1,
                raw_data_type: RawDataType::Uncalibrated,
                sample_minimum_chunk_size: 200,
                accepted_first_chunk_classifications: vec![83, 65],
                // May need to increase these for larger pore arrays
                action_stream_queue_buffer: 2048,
                dori_stream_queue_buffer: 2048,
                logging_queue_buffer: 4096
            },
            dori: DoriConfig {
                uds_path: get_env_var("STREAMFISH_DORI_UDS_PATH").into(),
                uds_path_override: get_env_var("STREAMFISH_DORI_UDS_PATH_OVERRIDE").trim().parse().unwrap(),
                dorado_path: get_env_var("STREAMFISH_DORI_DORADO_PATH").into(),
                dorado_args:  get_env_var("STREAMFISH_DORI_DORADO_ARGS"),

            }
        }
    }
}

impl std::fmt::Display for StreamfishConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = formatdoc! {"


        =======================
        Streamfish configuration
        =======================

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