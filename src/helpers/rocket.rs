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
            response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
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
        .raw_header("Access-Control-Allow-Origin", "*")
        .raw_header("Access-Control-Allow-Methods", "OPTIONS, POST, PUT, GET, DELETE")
        .raw_header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        .finalize()
}

#[cfg(feature = "webfrontend")]
pub fn factory(pool: super::db::Pool, config: config::Config) -> rocket::config::Result<Rocket> {
    use ::static_files;
    Ok(base_factory(pool, config)?
        .mount("/", routes![
               static_files::get_index,
               static_files::get_elmjs,
               static_files::get_sessionjs,
               static_files::get_audiojs,
               static_files::get_appcss,
               static_files::get_robotocss,
               static_files::get_materialcss,
        ]))
}

#[cfg(not(feature = "webfrontend"))]
pub fn factory(pool: super::db::Pool, config: config::Config) -> rocket::config::Result<Rocket> {
    base_factory(pool, config)
}

pub fn base_factory(pool: super::db::Pool, config: config::Config) -> rocket::config::Result<Rocket> {
    let rocket_config = Config::build(Environment::Production)
        .address(config.web.address.clone())
        .port(config.web.port.clone())
        .finalize()?;
    Ok(rocket::custom(rocket_config, true)
        .attach(CORS())
        .manage(pool)
        .manage(config.clone())
        .mount("/", routes![options_handler])
        .mount("/", routes![api::audiobooks::get_data_file])
        .mount("/api/", routes![
            api::libraries::libraries,
            api::libraries::all_the_things,
            api::libraries::update_playstates,
            api::audiobooks::get_coverart,
            api::audiobooks::get_audiobook,
            api::audiobooks::get_audiobooks,
        ])
        .mount("/api/auth/", routes![
               api::auth::login,
               api::auth::logout,
               api::auth::logout_all,
               api::auth::register,
               api::auth::whoami,
        ]))
}
