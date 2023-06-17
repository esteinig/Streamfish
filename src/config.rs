use clap::crate_version;

fn get_env_var(var: &str) -> String {
    std::env::var(var).expect(&format!("Failed to load environmental variable: {}", var))
}


#[derive(Debug, Clone)]
pub struct MinKnowConfig {
    host: String,
    port: i32
}


#[derive(Debug, Clone)]
pub struct ReadUntilConfig {
    
}

#[derive(Debug, Clone)]
pub struct ReefSquidConfig {
    version: String,
    minknow: MinKnowConfig,
    readuntil: ReadUntilConfig
}

impl ReefSquidConfig {
    pub fn new(dot_env: bool) -> ReefSquidConfig {

        if dot_env {
            dotenvy::dotenv().expect("Failed to load dotenv file from directory tree");
        }
        
        Self {
            version: crate_version!().to_string(),
            minknow: MinKnowConfig {
                host: get_env_var("REEFSQUID_MINKNOW_HOST"),
                port: get_env_var("REEFSQUID_MINKNOW_PORT").parse::<i32>().unwrap(),
            },
            readuntil: ReadUntilConfig {

            }
        }
    }
}