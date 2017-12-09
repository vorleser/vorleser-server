use models::user::User;
use rocket_contrib::Json;
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use helpers::uuid::Uuid;
use models::library::Library;
use models::playstate::Playstate;
use models::audiobook::Audiobook;
use diesel::prelude;
use std::path::{Path, PathBuf};
use api::ranged_file::RangedFile;
use std::fs;
use std::io;
use schema::audiobooks::dsl::{audiobooks, self};
use responses::{APIResponse, self, ok, internal_server_error};
use rocket::response::NamedFile;

#[get("/data/<book_id>")]
pub fn data_file(current_user: User, db: DB, book_id: Uuid) -> Result<RangedFile, APIResponse> {
    match current_user.get_book_if_accessible(&book_id, &*db)? {
        Some(_) => (),
        None => return Err(responses::not_found())
    };
    let book = audiobooks.filter(dsl::id.eq(book_id)).first::<Audiobook>(&*db)?;
    let mut path = PathBuf::from("data/");
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
pub fn get_coverart(current_user: User, db: DB, book_id: Uuid) -> Result<NamedFile, APIResponse> {
    use schema::libraries::dsl::*;
    let book = match current_user.get_book_if_accessible(&book_id, &*db)? {
        Some(a) => a,
        None => return Err(responses::not_found().message("No book found or not accessible."))
    };
    let mut path = PathBuf::from("data/img");
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
pub fn get_audiobooks(current_user: User, db: DB) -> Result<APIResponse, APIResponse> {
    use schema::libraries::dsl::*;
    let user_books = current_user.accessible_audiobooks(&*db)?;
    Ok(ok().data(json!(user_books)))
}

#[get("/audiobooks/<book_id>")]
pub fn audiobook(current_user: User, db: DB, book_id: Uuid) -> Result<APIResponse, APIResponse> {
    use schema::libraries::dsl::*;
    let book = match current_user.get_book_if_accessible(&book_id, &*db)? {
        Some(a) => a,
        None => return Ok(responses::not_found())
    };
    Ok(ok().data(json!(book)))
}
