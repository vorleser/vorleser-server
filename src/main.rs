#![feature(plugin)]
#![plugin(rocket_codegen)]
#![feature(libc)]
#![feature(custom_attribute)]

#[macro_use] extern crate lazy_static;
extern crate uuid;
extern crate rocket;
#[macro_use] extern crate rocket_contrib;
extern crate serde_json;
#[macro_use] extern crate serde_derive;
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

mod api;
mod validation;
mod models;
mod schema;
mod handlers;
mod responses;
mod helpers;

mod metadata;

use std::fs::File;
use std::io::Write;
use std::env;

use std::fs;
use std::path::Path;

fn main() {
    rocket::ignite()
        .manage(helpers::db::init_db_pool())
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
    scan_library(Path::new("test-data"));
}

fn scan_library(library_path: &Path) {
    //todo: it might be nice to check for file changed data and only check new files
    let dir = fs::read_dir(library_path).unwrap();
    dir.map(|entry| match entry {
            Ok(ref e) => {
                let metadata = e.metadata().unwrap();
                if metadata.is_dir() {
                    create_multifile_audiobook(&e.path());
                }
                else if metadata.is_file() {
                    create_audiobook(&e.path());
                }
            },
            Err(ref e) => println!("Error encountered reading file: {}", e)
    });
}


fn check_files(root: &Path) {

}

fn create_multifile_audiobook(path: &Path) -> Result<(), metadata::MediaError> {
    println!("Creating audiobook from dir");
    Ok(())
}

fn create_audiobook(path: &Path) -> Result<(), metadata::MediaError> {
    let md = try!(metadata::MediaFile::read_file(path)).get_mediainfo();
    println!("{:?}", md);
    Ok(())
}

fn save(buf: &[u8]) {
    let mut f = File::create("lul.jpg").unwrap();
    if let Ok(_) = f.write_all(buf) {
        println!("Successfully wrote image!")
    }
}

