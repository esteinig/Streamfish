
use std::path::PathBuf;

use indoc::formatdoc;
use clap::crate_version;

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


#[derive(Debug, Clone)]
pub struct ReadUntilConfig {
    
}

#[derive(Debug, Clone)]
pub struct DoriConfig {
    // Unix domain socket path
    // implemenmt TCP later in case of 
    // serving remotely
    pub uds_path: PathBuf,
    pub uds_path_override: bool,
}

#[derive(Debug, Clone)]
pub struct ReefsquidConfig {
    // Reefsquid version
    pub version: String,
    // Dori server configuration
    pub dori: DoriConfig,
    // MinKnow client configuration
    pub minknow: MinKnowConfig,
    // ReadUntil client configuration
    pub readuntil: ReadUntilConfig,
}

impl ReefsquidConfig {
    pub fn new(dot_env: bool) -> ReefsquidConfig {

        if dot_env {
            dotenvy::dotenv().expect("Could not find '.env' file in directory tree");
        }
        
        Self {
            version: crate_version!().to_string(),
            minknow: MinKnowConfig {
                host: get_env_var("REEFSQUID_MINKNOW_HOST"),
                port: get_env_var("REEFSQUID_MINKNOW_PORT").parse::<i32>().unwrap(),
                token: get_env_var("REEFSQUID_MINKNOW_TOKEN"),
                tls_cert_path: get_env_var("REEFSQUID_MINKNOW_TLS_CERT_PATH").into(),
            },
            readuntil: ReadUntilConfig {

            },
            dori: DoriConfig {
                uds_path: get_env_var("REEFSQUID_DORI_UDS_PATH").into(),
                uds_path_override: get_env_var("REEFSQUID_DORI_UDS_PATH_OVERRIDE").trim().parse().unwrap(),
            }
        }
    }
}

impl std::fmt::Display for ReefsquidConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = formatdoc! {"


        =======================
        Reefsquid configuration
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