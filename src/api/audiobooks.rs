use crate::models::user::User;
use rocket_contrib::json::Json;
use diesel::prelude::*;
use serde_json;
use crate::helpers::db::DB;
use crate::helpers::uuid::Uuid;
use crate::models::library::Library;
use crate::models::playstate::Playstate;
use crate::models::audiobook::Audiobook;
use diesel::prelude;
use std::path::{Path, PathBuf};
use crate::api::ranged_file::RangedFile;
use std::fs;
use std::io;
use crate::schema::audiobooks::dsl::{audiobooks, self};
use crate::responses::{APIResponse, APIError, self, ok, internal_server_error};
use rocket::response::NamedFile;
use crate::config::Config;

#[get("/data/<book_id>")]
pub fn get_data_file(current_user: User, db: DB, book_id: Uuid, config: Config) -> Result<RangedFile, APIError> {
    match current_user.get_book_if_accessible(&book_id, &*db)? {
        Some(_) => (),
        None => return Err(responses::not_found())
    };
    let book = audiobooks.filter(dsl::id.eq(book_id)).first::<Audiobook>(&*db)?;
    let mut path = PathBuf::from(config.data_directory);
    path.push(book.id.hyphenated().to_string());
    path.set_extension(book.file_extension);
    match RangedFile::open(path.clone()) {
        Ok(f) => Ok(f),
        Err(_) => {
            println!("Audiobook file not found in data directory: {:?}", path);
            Err(internal_server_error())
        }
    }
}

#[get("/coverart/<book_id>")]
pub fn get_coverart(current_user: User, db: DB, book_id: Uuid, config: Config) -> Result<NamedFile, APIError> {
    use crate::schema::libraries::dsl::*;
    let book = match current_user.get_book_if_accessible(&book_id, &*db)? {
        Some(a) => a,
        None => return Err(responses::not_found().message("No book found or not accessible."))
    };
    let mut path = PathBuf::from(config.data_directory);
    path.push("img");
    path.push(book_id.hyphenated().to_string());
    match NamedFile::open(path) {
        Ok(f) => Ok(f),
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => Err(responses::not_found().message("No cover art found.")),
            _ => Err(responses::internal_server_error())
        }
    }
}

#[get("/audiobooks")]
pub fn get_audiobooks(current_user: User, db: DB) -> Result<APIResponse, APIError> {
    use crate::schema::libraries::dsl::*;
    let user_books = current_user.accessible_audiobooks(&*db)?;
    Ok(ok().data(json!(user_books)))
}

#[get("/audiobooks/<book_id>")]
pub fn get_audiobook(current_user: User, db: DB, book_id: Uuid) -> Result<APIResponse, APIError> {
    use crate::schema::libraries::dsl::*;
    let book = match current_user.get_book_if_accessible(&book_id, &*db)? {
        Some(a) => a,
        None => return Err(responses::not_found())
    };
    Ok(ok().data(json!(book)))
}
