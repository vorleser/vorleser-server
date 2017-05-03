#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(custom_attribute)]
#![allow(dead_code)]
#![feature(pub_restricted)]

#[macro_use] extern crate lazy_static;
extern crate ring;
extern crate uuid;
extern crate rocket;
#[macro_use] extern crate rocket_contrib;
extern crate serde_json; #[macro_use] extern crate serde_derive;
extern crate validator;
#[macro_use] extern crate validator_derive;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
extern crate chrono;
extern crate argon2rs;
extern crate rustc_serialize;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;
extern crate regex;
extern crate walkdir;
extern crate dotenv;
extern crate image;
extern crate humanesort;

mod api;
mod validation;
mod models;
mod schema;
mod handlers;
mod responses;
mod helpers;
mod worker;

use std::fs::File;
use std::io::Write;

use worker::scanner::Scanner;
use std::env;
use regex::Regex;
use std::path::Path;
use schema::libraries;
use schema::libraries::dsl::*;
use models::library::Library;
use diesel::LoadDsl;
use diesel::prelude::*;

fn main() {
    let pool = helpers::db::init_db_pool();

    if env::args().nth(1) == Some("scan".to_string()) {
        let ref db = *pool.get().unwrap();
        let allLibraries = libraries.load::<Library>(db).unwrap();
        for l in allLibraries {
            println!("scanning library {}", l.location);
            let scanner = Scanner {
                regex: Regex::new("^[^/]+$").expect("Invalid Regex!"),
                library: l,
                pool: pool.clone(),
            };
            scanner.scan_library();
        }
        std::process::exit(0);
    }

    rocket::ignite()
        .manage(pool)
        .mount("/api/hello/", routes![api::hello::whoami])
        .mount("/api/auth/", routes![
               api::auth::login,
               api::auth::register,
        ])
        .catch(errors![handlers::bad_request_handler, handlers::unauthorized_handler,
                       handlers::forbidden_handler, handlers::not_found_handler,
                       handlers::internal_server_error_handler,
                       handlers::service_unavailable_handler])
        .launch();

}
