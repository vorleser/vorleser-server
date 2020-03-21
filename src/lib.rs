#![feature(plugin, non_ascii_idents, proc_macro_hygiene)]
#![allow(dead_code, unused)]
#![feature(decl_macro)]
#![feature(core_intrinsics)]
// We need the atomic mutex for locking ffmepg initialization
// pass by value is handy for use in rocket routes
// print literal will also warn in rocket routes
#![cfg_attr(feature = "cargo-clippy", allow(mutex_atomic, needless_pass_by_value, print_literal))]

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate failure;
#[macro_use(log, info, debug, warn, trace)]
extern crate log;
extern crate simplelog;

extern crate base64;
extern crate ring;
extern crate uuid;
#[macro_use] extern crate rocket;
extern crate clap;
#[macro_use] extern crate rocket_contrib;
extern crate serde_json; #[macro_use] extern crate serde_derive;
extern crate validator;
#[macro_use] extern crate validator_derive;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;
extern crate chrono;
extern crate argon2rs;
extern crate ffmpeg_sys as ffmpeg;
extern crate regex;
extern crate walkdir;
extern crate fs2;
extern crate image;
extern crate humanesort;
extern crate toml;
extern crate id3;
extern crate mp3_metadata;

#[cfg(test)] #[macro_use] extern crate speculate;

pub mod api;
pub mod validation;
pub mod models;
pub mod schema;
pub mod handlers;
pub mod responses;
pub mod helpers;
pub mod worker;
pub mod config;
#[cfg(feature = "webfrontend")]
pub mod static_files;
#[cfg(test)]
mod tests;

embed_migrations!("migrations");
