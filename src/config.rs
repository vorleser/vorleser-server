use std::sync::Arc;
use std::sync::Mutex;
use toml;
use std::path::Path;

lazy_static! {
    static ref _CONFIG: Mutex<Option<Config>> = Mutex::new(None);
}

pub fn init_config() {
    let config: Config = toml::from_str(r#"
        data_directory = 'data'
        register_web = false
    "#).unwrap();
    let mut guard = _CONFIG.lock().unwrap();
    *guard = Some(config);
}

pub fn init_config_from_path(path: &AsRef<Path>) {
    unimplemented!();
}

pub fn get_config() -> Config {
    let guard = _CONFIG.lock().unwrap();
    (*guard).clone().unwrap()
}

#[derive(Deserialize, Clone)]
pub struct Config {
    data_directory: String,
    register_web: bool
}
