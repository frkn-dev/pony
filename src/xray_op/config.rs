use log::{debug, warn};
use serde::Deserialize;
use std::error::Error;
use std::{collections::HashMap, fs::File, io::Read};

use crate::state::{inbound::Inbound as NodeInbound, tag::Tag};

#[derive(Debug, Deserialize, Clone)]
pub struct Inbound {
    pub tag: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct Api {
    pub listen: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub inbounds: Vec<Inbound>,
    pub api: Api,
}

impl Config {
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        for inbound in &self.inbounds {
            match inbound.tag.parse::<Tag>() {
                Ok(_) => {
                    debug!("Xray Config: Tag {} is valid", inbound.tag);
                    return Ok(());
                }

                Err(e) => {
                    let error = format!("Xray Config: Tag {:?} is invalid: {:?}", inbound.tag, e);
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }

    pub fn get_inbounds(&self) -> HashMap<Tag, NodeInbound> {
        let mut result_inbounds = HashMap::new();
        for inbound in &self.inbounds {
            match inbound.tag.parse::<Tag>() {
                Ok(tag) => {
                    result_inbounds.insert(tag, NodeInbound::new(inbound.port));
                    debug!("Xray Config: Tag {} inserted into HashMap", inbound.tag);
                }
                Err(_) => {
                    warn!(
                        "Xray Config: Tag {} is invalid and will be skipped",
                        inbound.tag
                    );
                }
            }
        }
        result_inbounds
    }
}

pub fn read_xray_config(file_path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();

    file.read_to_string(&mut contents)?;

    let config: Config = serde_json::from_str(&contents)?;

    Ok(config)
}
