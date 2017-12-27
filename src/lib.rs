#![feature(custom_attribute, plugin, non_ascii_idents, use_extern_macros)]
#![plugin(rocket_codegen)]
#![cfg_attr(test, plugin(stainless))]
#![allow(dead_code, unused)]
#![feature(decl_macro)]
// We need the atomic mutex for locking ffmepg initialization
// pass by value is handy for use in rocket routes
#![cfg_attr(feature = "cargo-clippy", allow(mutex_atomic, needless_pass_by_value))]

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate error_chain;
#[macro_use(log, info, debug, warn, trace)]
extern crate log;


extern crate base64;
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
#[macro_use] extern crate diesel_migrations;
extern crate chrono;
extern crate argon2rs;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate ffmpeg_sys as ffmpeg;
extern crate regex;
extern crate walkdir;
extern crate image;
extern crate humanesort;
extern crate toml;

pub mod api;
pub mod validation;
pub mod models;
pub mod schema;
pub mod handlers;
pub mod responses;
pub mod helpers;
pub mod worker;
pub mod config;
#[cfg(test)]
mod tests;

embed_migrations!("migrations");
