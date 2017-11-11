use std::sync::Arc;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io;
use std::io::{Write, Read};
use toml;
/// This module holds functions for loading config files.

#[cfg(release = "release")]
static CONFIG_LOCATION: &'static str = "/etc/vorleser.toml";

#[cfg(not(build = "release"))]
static CONFIG_LOCATION: &'static str = "default-config.toml";

error_chain! {
    foreign_links {
        Io(io::Error);
        Toml(toml::de::Error);
    }

    errors {
        Other(t: &'static str) {
            description(t)
        }
    }
}

/// Load a configuration, this checks xdg config paths.
/// `load_config_from_path` should be used when manually loading a specific file.
pub fn load_config() -> Result<Config> {
    load_config_from_path(&CONFIG_LOCATION)
}

pub fn load_config_from_path(config_path: &AsRef<Path>) -> Result<Config> {
    let mut file = File::open(config_path)?;
    let mut content: Vec<u8> = Vec::new();
    file.read_to_end(&mut content);
    let conf = toml::from_slice(&content)?;
    Ok(conf)
}

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_data_directory")]
    pub data_directory: String,
    #[serde(default)] // Default to false
    pub register_web: bool,
    pub web: WebConfig
}

#[derive(Deserialize, Clone)]
pub struct WebConfig {
    #[serde(default = "default_data_directory")]
    pub address: String,
    pub port: u16,
}

fn default_data_address() -> String {
    "localhost".to_owned()
}

fn default_data_directory() -> String {
    "data".to_owned()
}
