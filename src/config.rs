use std::sync::Arc;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io;
use std::io::{Write, Read};
use toml;
/// This module holds functions for loading config files.

lazy_static! {
    static ref _CONFIG: RwLock<Option<Config>> = RwLock::new(None);
}

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

static DEFAULT_CONFIG: &'static str = include_str!("../default-config.toml");


/// Load a configuration, this checks xdg config paths.
/// `load_config_from_path` should be used when manually loading a specific file.
pub fn load_config() -> Result<()> {
    load_config_from_path(&CONFIG_LOCATION)
}

pub fn load_config_from_path(config_path: &AsRef<Path>) -> Result<()> {
    let mut file = File::open(config_path)?;
    let mut content: Vec<u8> = Vec::new();
    file.read_to_end(&mut content)?;
    let config = toml::from_slice(&content)?;
    let mut guard = _CONFIG.write().expect("Error accessing shared config object.");
    *guard = Some(config);
    Ok(())
}

pub fn get_config() -> Config {
    if (*_CONFIG.read().unwrap()).is_none() {
        load_config().expect("Failed loading config.");
    }
    let guard = _CONFIG.read().unwrap();
    return (*guard).clone().expect("Config was not loaded!");
}

pub fn borrow_config() -> RwLockReadGuard<'static, Option<Config>> {
    if (*_CONFIG.read().unwrap()).is_none() {
        load_config().expect("Failed loading config.");
    }
    let guard = _CONFIG.read().unwrap();
    guard
}

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_data_directory")]
    data_directory: String,
    #[serde(default)] // Default to false
    register_web: bool
}

fn default_data_directory() -> String {
    "data".to_owned()
}
