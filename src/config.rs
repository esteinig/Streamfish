
use clap::crate_version;
use indoc::formatdoc;

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
    pub token: String
}


#[derive(Debug, Clone)]
pub struct ReadUntilConfig {
    
}

#[derive(Debug, Clone)]
pub struct ReefSquidConfig {
    // Reefsquid version
    pub version: String,
    // MinKno  configuration
    pub minknow: MinKnowConfig,
    // ReadUntil configuration
    pub readuntil: ReadUntilConfig
}

impl ReefSquidConfig {
    pub fn new(dot_env: bool) -> ReefSquidConfig {

        if dot_env {
            dotenvy::dotenv().expect("Could not find '.env' file in directory tree");
        }
        
        Self {
            version: crate_version!().to_string(),
            minknow: MinKnowConfig {
                host: get_env_var("REEFSQUID_MINKNOW_HOST"),
                port: get_env_var("REEFSQUID_MINKNOW_PORT").parse::<i32>().unwrap(),
                token: get_env_var("REEFSQUID_MINKNOW_TOKEN"),
            },
            readuntil: ReadUntilConfig {

            }
        }
    }
}

impl std::fmt::Display for ReefSquidConfig {
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