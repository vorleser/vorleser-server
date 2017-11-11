use rocket::{self, Rocket};
use ::api;
use ::handlers;

use rocket::{Request, Response};
use rocket::config::{Config, Environment};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Header, ContentType, Method};
use std::io::Cursor;
use std::path::PathBuf;

use config;
pub struct CORS();

impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to requests",
            kind: Kind::Response
        }
    }

    fn on_response(&self, request: &Request, response: &mut Response) {
        if request.method() == Method::Options || response.content_type() == Some(ContentType::JSON) {
            response.set_header(Header::new("Access-Control-Allow-Origin", "http://localhost:9901"));
            response.set_header(Header::new("Access-Control-Allow-Methods", "POST, GET, PUT, GET, DELETE"));
            response.set_header(Header::new("Access-Control-Allow-Headers", "Content-Type, Authorization"));
            // response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
        }

        if request.method() == Method::Options {
            response.set_header(ContentType::Plain);
            response.set_sized_body(Cursor::new(""));
        }
    }
}

#[route(OPTIONS, "/<path..>")]
fn options_handler<'a>(path: PathBuf) -> Response<'a> {
    Response::build()
        .raw_header("Access-Control-Allow-Origin", "http://localhost:9901")
        .raw_header("Access-Control-Allow-Methods", "OPTIONS, POST, PUT, GET, DELETE")
        .raw_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        .finalize()
}

pub fn factory(pool: super::db::Pool, config: config::Config) -> rocket::config::Result<Rocket> {
    let rocket_config = Config::build(Environment::Production)
        .address(config.web.address.clone())
        .port(config.web.port.clone())
        .finalize()?;
    Ok(rocket::custom(rocket_config, true)
        .attach(CORS())
        .manage(pool)
        .manage(config.clone())
        .mount("/", routes![options_handler])
        .mount("/", routes![api::audiobooks::data_file])
        .mount("/api/", routes![
            api::libraries::libraries,
            api::libraries::all_the_things,
            api::libraries::update_playstates,
            api::audiobooks::get_coverart,
            api::audiobooks::audiobook,
            api::audiobooks::get_audiobooks,
        ])
        .mount("/api/auth/", routes![
               api::auth::login,
               api::auth::register,
               api::auth::whoami,
        ]))
}
