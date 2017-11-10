use std::sync::Arc;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io;
use std::io::{Write, Read};
use toml;
/// This module holds functions for loading config files.

static mut _CONFIG: Option<Config> = None;

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
pub fn load_config() -> Result<()> {
    load_config_from_path(&CONFIG_LOCATION)
}

pub fn load_config_from_path(config_path: &AsRef<Path>) -> Result<()> {
    unsafe {
        if _CONFIG.is_some() {
            panic!("Trying to load config for a second time.");
        }
    }
    let mut file = File::open(config_path)?;
    let mut content: Vec<u8> = Vec::new();
    file.read_to_end(&mut content)?;
    unsafe {
        _CONFIG = Some(toml::from_slice(&content)?);
    };
    Ok(())
}

pub fn get_config() -> &'static Config {
    unsafe {
        _CONFIG.as_ref().unwrap()
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_data_directory")]
    pub data_directory: String,
    #[serde(default)] // Default to false
    pub register_web: bool
}

fn default_data_directory() -> String {
    "data".to_owned()
}
