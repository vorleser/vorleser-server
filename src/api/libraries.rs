use models::user::UserModel;
use responses::{APIResponse, ok};
use rocket_contrib::JSON;
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use models::library::Library;
use models::audiobook::Audiobook;
use models::chapter::Chapter;
use models::audiobook::Playstate;

#[get("/libraries")]
pub fn libraries(current_user: UserModel, db: DB) -> APIResponse {
    use schema::libraries::dsl::*;
    let libs = libraries.load::<Library>(&*db).unwrap();
    ok().data(json!(libs))
}

#[get("/all_the_things")]
pub fn all_the_things(current_user: UserModel, db: DB) -> APIResponse {
    use schema;
    let libs = schema::libraries::dsl::libraries.load::<Library>(&*db).unwrap();
    let books = schema::audiobooks::dsl::audiobooks.load::<Audiobook>(&*db).unwrap();
    let chapters = schema::chapters::dsl::chapters.load::<Chapter>(&*db).unwrap();
    ok().data(json!({
        "libraries": libs,
        "books": books,
        "chapters": chapters,
    }))
}
