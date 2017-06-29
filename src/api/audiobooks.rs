use models::user::UserModel;
use responses::{APIResponse, ok};
use rocket_contrib::{JSON, UUID};
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use models::library::Library;
use models::playstate::Playstate;
use diesel::prelude;

#[get("/audiobooks/<book_id>")]
pub fn audiobook(current_user: UserModel, db: DB, book_id: UUID) -> APIResponse {
    use schema::libraries::dsl::*;
    // Audiobook::acessible_by(current_user).load(&*DB);
    // let libs = audiobooks.load::<Library>(&*db).unwrap();
    ok()
}
