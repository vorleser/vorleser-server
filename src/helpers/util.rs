use rocket::config::{Config, Environment, self};

pub fn get_secret<'a>() -> String {
    const DEFAULT_SECRET: &'static str = "secret";
    let conf = config::active().expect("No config found.");
    match (conf.get_str("secret"), Environment::active().unwrap()) {
        (Ok(s), _) => s.to_string(),
        (Err(_), Environment::Development) => DEFAULT_SECRET.to_string(),
        (Err(_), _) => panic!("A secret needs to be set unless you are in a development environment!")
    }
}
