use std::sync::Arc;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io;
use std::io::{Write, Read};
use toml;
use rocket::request::{self, FromRequest};
use simplelog::LevelFilter;
use rocket::{Request, State, Outcome};
use failure::Error;
/// This module holds functions for loading config files.

#[cfg(not(debug_assertions))]
static CONFIG_LOCATIONS: &'static [&'static str] = &["/etc/vorleser.toml"];

#[cfg(debug_assertions)]
static CONFIG_LOCATIONS: &'static [&'static str] = &["vorleser-dev.toml"];

#[derive(Debug, Fail)]
enum ConfigError {
    #[fail(display = "Could not read any config files")]
    NoReadableConfig
}

/// Load a configuration, this checks xdg config paths.
/// `load_config_from_path` should be used when manually loading a specific file.
pub fn load_config() -> Result<Config, Error> {
    for location in CONFIG_LOCATIONS.iter() {
        let conf = load_config_from_path(&location);
        if conf.is_ok() {
            debug!("Using config from: {}", location);
            return conf;
        }
    }
    Err(ConfigError::NoReadableConfig.into())
}

pub fn load_config_from_path(config_path: &dyn AsRef<Path>) -> Result<Config, Error> {
    let mut file = File::open(config_path)?;
    let mut content: Vec<u8> = Vec::new();
    file.read_to_end(&mut content)?;
    let conf = toml::from_slice(&content)?;
    Ok(conf)
}

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_data_directory")]
    pub data_directory: String,
    #[serde(default)] // Default to false
    pub register_web: bool,
    pub database: String,
    pub web: WebConfig,
    pub scan: ScanConfig,
    pub sentry_dsn: Option<String>,
    pub logging: LoggingConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LoggingConfig {
    pub level: String,
    #[serde(default)]
    pub file: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ScanConfig {
    #[serde(default)] // default to false
    pub enabled: bool,
    #[serde(default= "default_scan_interval")]
    pub interval: u64,
}

#[derive(Deserialize, Clone)]
pub struct WebConfig {
    #[serde(default="default_data_directory")]
    pub address: String,
    pub port: u16,
    #[serde(default)] // default to false
    pub debug: bool,
}

fn default_log_level() -> String {
    "info".to_owned()
}

fn default_scan_interval() -> u64 {
    600
}

fn default_data_address() -> String {
    "localhost".to_owned()
}

fn default_data_directory() -> String {
    "data".to_owned()
}

fn default_log_location() -> Option<String> {
    let mut path = default_data_directory();
    path.push_str("/vorleser.log");
    Some(path)
}

impl<'a, 'r> FromRequest<'a, 'r> for Config {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<Config, ()> {
        request.guard::<State<Config>>()
            .map(|config| config.clone())
    }
}
