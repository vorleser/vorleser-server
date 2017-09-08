use models::user::UserModel;
use responses::{APIResponse, ok};
use rocket_contrib::{Json, UUID};
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use models::library::Library;
use models::playstate::Playstate;
use diesel::prelude;
use std::path::Path;
use api::ranged_file::RangedFile;
use std::fs;

#[get("/data/<book_id>")]
pub fn data_file(db: DB, book_id: UUID) -> RangedFile {
    let idstr = book_id.hyphenated().to_string();
    for p in fs::read_dir("data/").unwrap() {
        let entry = p.unwrap();
        if entry.file_name().to_str().unwrap().starts_with(&idstr) {
            return RangedFile::open(entry.path()).unwrap()
        }
    }
    unreachable!()
}

#[get("/audiobooks/<book_id>")]
pub fn audiobook(current_user: UserModel, db: DB, book_id: UUID) -> APIResponse {
    use schema::libraries::dsl::*;
    // Audiobook::acessible_by(current_user).load(&*DB);
    // let libs = audiobooks.load::<Library>(&*db).unwrap();
    ok()
}
