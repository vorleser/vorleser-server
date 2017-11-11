#![allow(dead_code, unused)]
#![feature(decl_macro)]
// We need the atomic mutex for locking ffmepg initialization
// pass by value is handy for use in rocket routes
#![cfg_attr(feature = "cargo-clippy", allow())]

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
extern crate clap;
extern crate regex;
extern crate vorleser_server;
extern crate diesel;

use std::error::Error;
use std::path::PathBuf;
use vorleser_server::worker::scanner::Scanner;
use regex::Regex;
use vorleser_server::schema::libraries;
use vorleser_server::schema::libraries::dsl::*;
use vorleser_server::models::library::{Library, NewLibrary};
use vorleser_server::models::user::{UserModel, NewUser};
use vorleser_server::schema::users;
use vorleser_server::config;
use diesel::LoadDsl;
use diesel::prelude::*;
use clap::{Arg, App, SubCommand};
use diesel::insert;
use vorleser_server::helpers::db::{Pool, init_db_pool};
use vorleser_server::helpers;

static PATH_REGEX: &'static str = "^[^/]+$";

fn main() {
    env_logger::init().unwrap();
    let pool = init_db_pool();

    let matches = App::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(SubCommand::with_name("serve"))
        .subcommand(SubCommand::with_name("scan")
            .arg(Arg::with_name("full")
                 .long("full")
                 .help("Perform a full scan, not an incremental one")
            )
        )
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
        .subcommand(SubCommand::with_name("create_library")
            .about("Create a new Library")
            .arg(Arg::with_name("path")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("regex")
                .takes_value(true)
                .default_value(PATH_REGEX)
            )
        ).arg(Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .takes_value(true)
        )
        .subcommand(SubCommand::with_name("sample-config")
            .about("Print the default configuration file to stdout.")
        )
        .get_matches();

    let config_result = if let Some(config_path) = matches.value_of("config") {
        config::load_config_from_path(&config_path)
    } else {
        config::load_config()
    };

    if let Err(e) = config_result {
        use config::Error as Error;
        use config::ErrorKind;
        match e {
            Error(ErrorKind::Io(e), _) => error_log!("IO error reading configuration file: {}", e),
            Error(ErrorKind::Toml(e), _) => error_log!("Malformed configuration file: {}", e),
            _ => error_log!("Unknown error reading configuration file.")
        }
        panic!("Error loading config. Try using --config to supply a valid configuration file.")
    } else {
        info!("Succeeded loading config!")
    }
    let conf = config_result.unwrap();

    if let Some(new_command) = matches.subcommand_matches("create_library") {
        let conn = &*pool.get().unwrap();
        let input_path = PathBuf::from(new_command.value_of("path").expect("Please provide a valid utf-8 path."));
        let regex = new_command.value_of("regex").expect("Regex needs to be valid utf-8.");
        let path = if input_path.is_absolute() {
            input_path
        } else {
            std::env::current_dir().expect("No working directory.").join(input_path)
        };
        match Regex::new(regex) {
            Ok(_) => {
                match Library::create(path.to_string_lossy().into_owned(), regex.to_owned(), &*conn)
                {
                    Ok(lib) => info!("Successfully created library."),
                    Err(error) => error_log!("Library creation failed: {:?}", error.description())
                }
            },
            Err(e) => error_log!("Invalid regex: {:?}", e)
        }
        std::process::exit(0);
    };

    if let Some(cmd) = matches.subcommand_matches("sample-config") {
        print!(include_str!("../../default-config.toml"));
    }

    if let Some(scan) = matches.subcommand_matches("scan") {
        let db = &*pool.get().unwrap();
        let all_libraries = libraries.load::<Library>(db).unwrap();
        for l in all_libraries {
            let mut scanner = Scanner {
                regex: Regex::new(&l.is_audiobook_regex).expect("Invalid Regex!"),
                library: l,
                pool: pool.clone(),
                config: conf.clone()
            };

            let scan_result = if scan.is_present("full") {
                scanner.full_scan()
            } else {
                scanner.incremental_scan()
            };

            if let Err(error) = scan_result {
                error_log!("Scan failed with error: {:?}", error.description());
            } else {
                info!("Scan succeeded!");
            }
        }
        std::process::exit(0);
    }

    if let Some(create_user) = matches.subcommand_matches("create_user") {
        let db = &*pool.get().unwrap();

        let email = create_user.value_of("email").expect("a man has no name");
        let pass = create_user.value_of("password").expect("a man has no password");
        let user = UserModel::create(&email, &pass, db).expect("Error saving user");
    }

    if let Some(_) = matches.subcommand_matches("serve") {
        match helpers::rocket::factory(pool, conf) {
            Ok(r) => error_log!("{}", r.launch()),
            Err(e) => error_log!("Invalid web-server configuration: {}", e)
        };
    }

}
