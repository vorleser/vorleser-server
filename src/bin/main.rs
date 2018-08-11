#![allow(dead_code, unused)]
#![feature(decl_macro)]
// We need the atomic mutex for locking ffmepg initialization
// pass by value is handy for use in rocket routes
#![cfg_attr(feature = "cargo-clippy", allow())]

#[macro_use(log, info, debug, warn, trace)] extern crate log;

extern crate simplelog;
extern crate clap;
extern crate regex;
extern crate vorleser_server;
extern crate diesel;
extern crate sentry;
extern crate scheduled_thread_pool;

use std::error::Error;
use std::path::PathBuf;
use std::panic;
use std::panic::AssertUnwindSafe;
use std::time::Duration;
use std::fs::OpenOptions;

use sentry::integrations::panic::register_panic_handler;
use sentry::integrations::failure::capture_error;
use sentry::integrations::log::LoggerOptions;
use diesel::prelude::*;
use clap::{Arg, App, SubCommand, ArgMatches};
use regex::Regex;
use log::error as error_log;
use simplelog::{SimpleLogger, WriteLogger, CombinedLogger, TermLogger, LevelFilter};
use scheduled_thread_pool::ScheduledThreadPool;

use vorleser_server::worker::scanner::{Scanner, LockingBehavior};
use vorleser_server::schema::libraries;
use vorleser_server::schema::libraries::dsl::*;
use vorleser_server::models::library::Library;
use vorleser_server::models::user::{User, NewUser};
use vorleser_server::schema::users;
use vorleser_server::config::{self, Config, WebConfig, LoggingConfig};
use vorleser_server::helpers::db::{Pool, init_db_pool, init_db};
use vorleser_server::helpers;

static PATH_REGEX: &'static str = "^[^/]+$";

fn main() {
    let command_parser = build_command_parser();
    let matches = command_parser.get_matches();

    if let Some(cmd) = matches.subcommand_matches("sample-config") {
        print!(include_str!("../../vorleser-default.toml"));
        std::process::exit(0);
    }

    let mut conf = load_config(&matches);
    if let Some(level) = matches.value_of("log-level") {
        conf.logging.level = level.to_owned();
    }

    let sentry_guard = match conf.sentry_dsn {
        Some(ref dsn) => Some(init_sentry(dsn)),
        None => None,
    };

    init_logging(&conf.logging);

    init_db(conf.database.clone());
    let pool = init_db_pool(conf.database.clone());

    if let Some(new_command) = matches.subcommand_matches("create-library") {
        let conn = &*pool.get().unwrap();
        create_library(new_command, conn);
        std::process::exit(0);
    };

    if let Some(scan_match) = matches.subcommand_matches("scan") {
        run_scan_command(scan_match, &pool, &conf);
        std::process::exit(0);
    }

    if let Some(create_user) = matches.subcommand_matches("create-user") {
        let db = &*pool.get().unwrap();

        let email = create_user.value_of("email").expect("a man has no name");
        let pass = create_user.value_of("password").expect("a man has no password");
        let user = User::create(&email, &pass, db).expect("Error saving user");
    }


    if let Some(serve) = matches.subcommand_matches("serve") {
        let scan_thread_pool = ScheduledThreadPool::new(1);
        if conf.scan.enabled {
            let scan_db_pool = pool.clone();
            let scan_config = conf.clone();
            scan_thread_pool.execute_with_fixed_delay(
                Duration::new(10, 0),
                Duration::new(conf.scan.interval, 0),
                move || {
                    scan_job(scan_db_pool.clone(), scan_config.clone());
                }
            );
        }
        if let Some(port_string) = serve.value_of("port") {
            let port = port_string.parse::<u16>().expect("Invalid value for port.");
            conf = Config {
                web: WebConfig {
                    port: port,
                    .. conf.web
                },
                .. conf
            };
        }
        match helpers::rocket::factory(pool, conf) {
            Ok(r) => error_log!("{}", r.launch()),
            Err(e) => error_log!("Invalid web-server configuration: {}", e)
        };
    }

    if let Some(cmd) = matches.subcommand_matches("mlltify") {
        let path_str = cmd.value_of("file").expect("gib file");
        let path = std::path::Path::new(path_str);
        let res = helpers::mllt::mlltify(path);
        println!("{:?}", res);
    }

}

fn build_command_parser<'a, 'b>() -> App<'a, 'b> {
    App::new(env!("CARGO_PKG_NAME"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(SubCommand::with_name("serve")
            .arg(Arg::with_name("port")
                 .long("port")
                 .help("Which port to serve on. Overwrites the value in the config file.")
                 .takes_value(true)
            )
        )
        .subcommand(SubCommand::with_name("scan")
            .arg(Arg::with_name("full")
                 .long("full")
                 .help("Perform a full scan, not an incremental one")
            )
        )
        .subcommand(SubCommand::with_name("create-user")
            .arg(Arg::with_name("email")
                .takes_value(true)
                .required(true)
            )
            .arg(Arg::with_name("password")
                .takes_value(true)
                .required(true)
            )
        )
        .subcommand(SubCommand::with_name("create-library")
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
        ).arg(Arg::with_name("log-level")
                .short("l")
                .long("log-level")
                .value_name("LOG_LEVEL")
                .takes_value(true)
        )
        .subcommand(SubCommand::with_name("sample-config")
            .about("Print the default configuration file to stdout.")
        )
        .subcommand(SubCommand::with_name("mlltify")
            .arg(Arg::with_name("file").index(1))
        )
}

fn init_sentry(dsn: &str) -> sentry::internals::ClientInitGuard {
    let sentry_guard = sentry::init(dsn);
    register_panic_handler();
    return sentry_guard;
}

fn load_config(matches: &ArgMatches) -> Config {
    let config_result = if let Some(config_path) = matches.value_of("config") {
        config::load_config_from_path(&config_path)
    } else {
        config::load_config()
    };

    if let Err(e) = config_result {
        error_log!("Error loading config: {}", e);
        panic!("Error loading config. Try using --config to supply a valid configuration file.\nYou can get a default config file with the sample-config subcommand.");
    } else {
        println!("Succeeded loading config!")
    }
    config_result.unwrap()
}

fn create_library(command: &ArgMatches, conn: &SqliteConnection) {
    let input_path = PathBuf::from(
        command.value_of("path").expect("Please provide a valid utf-8 path.")
    );
    let regex = command.value_of("regex").expect("Regex needs to be valid utf-8.");
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
}

fn run_scan_command(command: &ArgMatches, pool: &Pool, config: &Config) {
    run_scan(pool, config, command.is_present("full"));
}

fn run_scan(pool: &Pool, config: &Config, full_scan: bool) {
    let conn = &*pool.get().unwrap();
    let all_libraries = libraries.load::<Library>(conn).unwrap();
    for l in all_libraries {
        let mut scanner = Scanner {
            regex: Regex::new(&l.is_audiobook_regex).expect("Invalid Regex!"),
            library: l,
            pool: pool.clone(),
            config: config.clone()
        };

        let scan_result = if full_scan {
            scanner.full_scan(LockingBehavior::Block)
        } else {
            scanner.incremental_scan(LockingBehavior::Block)
        };

        if let Err(error) = scan_result {
            capture_error(&error);
            error_log!("Scan failed with error: {}", error);
            error_log!("Backtrace: {}", error.backtrace());
        } else {
            info!("Scan succeeded!");
        }
    }
}

fn scan_job(pool: Pool, config: Config) {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_scan(&pool, &config, false);
    }));
    info!("Completed scan, result is: {:?}", result);
}

fn init_logging(config: &LoggingConfig) {
    let level = match config.level.to_lowercase().as_str() {
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            "off" => LevelFilter::Off,
            _ => LevelFilter::Info,
    };
    let mut loggers: Vec<Box<simplelog::SharedLogger>> = Vec::new();
    let term_logger = TermLogger::new(level, simplelog::Config::default());
    if let Some(logger) = term_logger {
        loggers.push(logger)
    } else {
        loggers.push(
            SimpleLogger::new(level, simplelog::Config::default())
        );
    }
    if let Some(ref file_path) = config.file {
        let file = OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(file_path)
            .expect("Unable to open log file for writing.");
        loggers.push(
            WriteLogger::new(level, simplelog::Config::default(), file)
        );
    }
    let combined = CombinedLogger::new(loggers);
    sentry::integrations::log::init(
        Some(combined),
        LoggerOptions {
            global_filter: None,
            filter: level,
            emit_breadcrumbs: true,
            emit_error_events: false,
            emit_warning_events: false,
        }
    );
}
