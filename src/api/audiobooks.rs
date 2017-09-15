use models::user::UserModel;
use rocket_contrib::{Json, UUID};
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use uuid::Uuid;
use models::library::Library;
use models::playstate::Playstate;
use models::audiobook::Audiobook;
use diesel::prelude;
use std::path::{Path, PathBuf};
use api::ranged_file::RangedFile;
use std::fs;
use schema::audiobooks::dsl::{audiobooks, self};
use responses::{APIResponse, self, ok, internal_server_error};

#[get("/data/<book_id>")]
pub fn data_file(db: DB, book_id: UUID) -> Result<RangedFile, APIResponse> {
    let idstr = book_id.hyphenated().to_string();
    let id = Uuid::parse_str(&idstr)?;
    let book = audiobooks.filter(dsl::id.eq(id)).first::<Audiobook>(&*db)?;
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

#[get("/audiobooks/<book_id>")]
pub fn audiobook(current_user: UserModel, db: DB, book_id: UUID) -> APIResponse {
    use schema::libraries::dsl::*;
    // Audiobook::acessible_by(current_user).load(&*DB);
    // let libs = audiobooks.load::<Library>(&*db).unwrap();
    ok()
}
