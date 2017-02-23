#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(libc)]
#![feature(custom_attribute)]
#![allow(dead_code)]
#![feature(pub_restricted)]

#[macro_use] extern crate lazy_static;
extern crate uuid;
extern crate rocket;
#[macro_use] extern crate rocket_contrib;
extern crate serde_json; #[macro_use] extern crate serde_derive;
extern crate validator;
#[macro_use] extern crate validator_derive;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_codegen;
extern crate jsonwebtoken;
extern crate chrono;
extern crate argon2rs;
extern crate rustc_serialize;
extern crate r2d2;
extern crate r2d2_diesel;
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;
extern crate regex;
extern crate walkdir;

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
use std::env;

use std::fs;
use std::path::{Path, PathBuf};

use std::thread;
use std::time;

use walkdir::{WalkDir, WalkDirIterator};
use regex::Regex;

use diesel::pg::PgConnection;
use std::env::args;
use worker::mediafile::{MediaFile, NewMediaFile};
use worker::error::*;

fn main() {
    let mut args = env::args();
    args.next();
    let mut lol: Vec<MediaFile> = args.map(
        |name| MediaFile::read_file(Path::new(&name)).unwrap()
        ).collect();
    // let stream = lol.first().unwrap().get_first_audio_stream().unwrap();
    match worker::mediafile::merge_files(Path::new("muxed.m4a"), lol) {
        Err(e) => println!("{}", e),
        _ => println!("Success")
    }

    return;

    let pool = helpers::db::init_db_pool();
    {
        let pool = pool.clone();
        thread::spawn(move || {
            let conn = pool.get().unwrap();
            let scanner = Scanner {
                regex: Regex::new("^[^/]+$").expect("Invalid Regex!"),
                path: Path::new("test-data").to_path_buf(),
                conn: &*conn,
            };
            loop {
                scanner.scan_library();
                thread::sleep(time::Duration::from_secs(5));
            }
        });
    }
    let mut args = env::args();
    args.next();
    // for s in args {
    //     println!("{}", s);
    //     match metadata::MediaFile::read_file(&s) {
    //         Ok(ref mut c) => {
    //             println!("{:?}", c.get_mediainfo());
    //             save(c.get_cover_art());
    //         },
    //         Err(e) => println!("Error: {}", e)
    //     }
    // }
    // let worker = thread::
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

struct Scanner<'a> {
    regex: Regex,
    path: PathBuf,
    conn: &'a PgConnection
}

impl<'a> Scanner<'a> {
    // pub fn new(conn: PgConnection, root: PathBuf, regex: Regex) {
    // }

    pub fn scan_library(&self) {
        //todo: it might be nice to check for file changed data and only check new files
        println!("Scanning library.");
        let mut walker = WalkDir::new(&self.path).follow_links(true).into_iter();
        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(e)) => panic!("Error: {}", e),
                Some(Ok(i)) => i,
            };
            let path = entry.path().strip_prefix(&self.path).unwrap();
            if path.components().count() == 0 { continue };
            if is_audiobook(path, &self.regex) {
                println!("{:?}", path);
                if path.is_dir() {
                    walker.skip_current_dir();
                }
            }
        }
    }
}

fn is_audiobook(path: &Path, regex: &Regex) -> bool {
    regex.is_match(path.to_str().unwrap())
}



fn create_multifile_audiobook(path: &Path) -> Result<(), MediaError> {
    println!("Creating audiobook from dir");
    Ok(())
}

fn create_audiobook(path: &Path) -> Result<(), MediaError> {
    println!("Creating audiobook!");
    let md = try!(MediaFile::read_file(path)).get_mediainfo();
    println!("{:?}", md);
    Ok(())
}

fn save(buf: &[u8]) {
    let mut f = File::create("lul.jpg").unwrap();
    if let Ok(_) = f.write_all(buf) {
        println!("Successfully wrote image!")
    }
}
