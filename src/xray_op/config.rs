use log::{debug, warn};
use serde::Deserialize;
use std::{fs::File, io::Read, str::FromStr};

use crate::xray_op::Tag;

#[derive(Debug, Deserialize, Clone)]
pub struct Inbound {
    pub tag: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub inbounds: Vec<Inbound>,
}

impl Config {
    pub fn validate(&self) {
        for inbound in &self.inbounds {
            match inbound.tag.parse::<Tag>() {
                Ok(_) => debug!("Xray Config: Tag {} is valid", inbound.tag),
                Err(_) => warn!("Xray Config: Tag {} is invalid", inbound.tag),
            }
        }
    }

    pub fn get_inbounds(&self) -> Vec<Tag> {
        <Vec<Inbound> as Clone>::clone(&self.inbounds)
            .into_iter()
            .filter_map(|i| Tag::from_str(&i.tag).ok())
            .collect()
    }
}

pub fn read_xray_config(file_path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(file_path)?;
    let mut contents = String::new();

    file.read_to_string(&mut contents)?;

    let config: Config = serde_json::from_str(&contents)?;

    Ok(config)
}
