use std::sync::Arc;
use std::sync::Mutex;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io;
use std::io::{Write, Read};
use toml;
use xdg;
/// This module holds functions for loading config files.
/// Make sure to call one of `load_config_from_path` or `load_config` exactly once.
/// These functions initialze a global variable that you should access via only `get_config`.
/// Calling `get_config` before `load_config` will panic the program.

lazy_static! {
    static ref _CONFIG: Mutex<Option<Config>> = Mutex::new(None);
}

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
    let xdg_result = build_xdg_config();
    if let Ok(conf_path) = xdg_result {
        load_config_from_path(&conf_path);
    } else {
        let config = toml::from_str(DEFAULT_CONFIG)?;
        let mut guard = _CONFIG.lock().expect("Error accessing shared config object.");
        *guard = Some(config);
    };
    Ok(())
}


/// Initialize xdg config file
/// Ensures a config file is placed in the xdg config directory
pub fn build_xdg_config() -> Result<PathBuf> {
    let xdg_dirs = xdg::BaseDirectories::with_prefix(env!("CARGO_PKG_NAME")).unwrap();
    let config_path = xdg_dirs.place_config_file("config.toml")
                          .expect("cannot create configuration directory");
    if !config_path.exists() {
        place_default_config_file(&config_path)?;
    }
    Ok(config_path)
}

fn place_default_config_file(path: &AsRef<Path>) -> Result<()> {
    let mut config_file = File::create(path.as_ref())?;
    config_file.write_all(DEFAULT_CONFIG.as_bytes())?;
    Ok(())
}

pub fn load_config_from_path(config_path: &AsRef<Path>) -> Result<()> {
    if !config_path.as_ref().exists() {
        place_default_config_file(&config_path)?;
    }
    let mut file = File::open(config_path)?;
    let mut content: Vec<u8> = Vec::new();
    file.read_to_end(&mut content)?;
    let config = toml::from_slice(&content).unwrap();
    let mut guard = _CONFIG.lock().expect("Error accessing shared config object.");
    *guard = Some(config);
    Ok(())
}

pub fn get_config() -> Config {
    let guard = _CONFIG.lock().unwrap();
    (*guard).clone().expect("Config was not loaded!")
}

#[derive(Deserialize, Clone)]
pub struct Config {
    data_directory: String,
    register_web: bool
}
