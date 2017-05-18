#![feature(custom_attribute, plugin)]
#![plugin(rocket_codegen)]
#![allow(dead_code, unused)]

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate error_chain;
#[macro_use(log, info, debug, warn, trace)] extern crate log;
// disgusting workaround for error also being present in rocket
#[macro_use]
macro_rules! error_log {
    (target: $target:expr, $($arg:tt)*) => (
        log!(target: $target, ::log::LogLevel::Error, $($arg)*);
    );
    ($($arg:tt)*) => (
        log!(::log::LogLevel::Error, $($arg)*);
    )
}

extern crate env_logger;
extern crate ring;
extern crate uuid;
extern crate rocket;
extern crate clap;
#[macro_use] extern crate rocket_contrib;
extern crate serde_json; #[macro_use] extern crate serde_derive;
extern crate validator;
#[macro_use] extern crate validator_derive;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
extern crate chrono;
extern crate argon2rs;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;
extern crate regex;
extern crate walkdir;
extern crate dotenv;
extern crate image;
extern crate humanesort;
extern crate serde;

mod api;
mod validation;
mod models;
mod schema;
mod handlers;
mod responses;
mod helpers;
mod worker;

use std::error::Error;
use worker::scanner::Scanner;
use regex::Regex;
use schema::libraries;
use schema::libraries::dsl::*;
use models::library::{Library, NewLibrary};
use models::user::{UserModel, NewUser};
use schema::users;
use diesel::LoadDsl;
use diesel::prelude::*;
use clap::{Arg, App, SubCommand};
use diesel::insert;

static PATH_REGEX: &'static str = "^[^/]+$";

fn main() {
    let pool = helpers::db::init_db_pool();

    let matches = App::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(SubCommand::with_name("serve"))
        .subcommand(SubCommand::with_name("scan"))
        .subcommand(SubCommand::with_name("create_user")
            .arg(Arg::with_name("email")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("password")
                .takes_value(true)
                .required(true)
            )
        )
        .subcommand(SubCommand::with_name("new")
            .about("Create a new Library")
            .arg(Arg::with_name("path")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("regex")
                .takes_value(true)
                .default_value(PATH_REGEX)
            )
        )
        .get_matches();

    if let Some(new_command) = matches.subcommand_matches("new") {
        env_logger::init().unwrap();
        let conn = &*pool.get().unwrap();
        let path = new_command.value_of("path").expect("Please provide a valid utf-8 path.");
        let regex = new_command.value_of("regex").expect("Regex needs to be valid utf-8.");
        match Regex::new(regex) {
            Ok(_) => {
                match insert(
                    &NewLibrary{
                        location: path.to_owned(),
                        is_audiobook_regex: regex.to_owned()
                    }).into(libraries::table).execute(&*conn)
                {
                    Ok(1) => info!("Successfully created library."),
                    _ => error_log!("Library creation failed.")
                }
            },
            Err(e) => error_log!("Invalid regex: {:?}", e)
        }
        std::process::exit(0);
    };

    if let Some(_) = matches.subcommand_matches("scan") {
        env_logger::init().unwrap();
        let db = &*pool.get().unwrap();
        let all_libraries = libraries.load::<Library>(db).unwrap();
        for l in all_libraries {
            let mut scanner = Scanner {
                regex: Regex::new(&l.is_audiobook_regex).expect("Invalid Regex!"),
                library: l,
                pool: pool.clone(),
            };
            if let Err(error) = scanner.scan_library() {
                error_log!("Scan failed with error: {:?}", error.description());
            } else {
                info!("Scan succeeded!");
            }
        }
        std::process::exit(0);
    }

    if let Some(create_user) = matches.subcommand_matches("create_user") {
        env_logger::init().unwrap();
        let db = &*pool.get().unwrap();

        let email = create_user.value_of("email").expect("a man has no name");
        let pass = create_user.value_of("password").expect("a man has no password");

        let new_password_hash = UserModel::make_password_hash(pass);
        let new_user = NewUser {
            email: email.to_string(),
            password_hash: new_password_hash,
        };

        let user = diesel::insert(&new_user)
            .into(users::table)
            .get_result::<UserModel>(&*db)
            .expect("Error saving user");
    }

    if let Some(_) = matches.subcommand_matches("serve") {
        rocket::ignite()
            .manage(pool)
            .mount("/api/", routes![
                api::libraries::libraries,
                api::libraries::all_the_things,
                api::libraries::update_playstate,
            ])
            .mount("/api/auth/", routes![
                   api::auth::login,
                   api::auth::register,
            ])
            .catch(errors![
                handlers::bad_request_handler,
                handlers::unauthorized_handler,
                handlers::forbidden_handler,
                handlers::not_found_handler,
                handlers::internal_server_error_handler,
                handlers::service_unavailable_handler
            ])
            .launch();
    }

}
