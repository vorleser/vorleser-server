use models::user::UserModel;
use responses::{APIResponse, ok};
use rocket_contrib::JSON;
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use models::library::Library;
use models::audiobook::Audiobook;
use models::chapter::Chapter;
use models::playstate::Playstate;

#[get("/libraries")]
pub fn libraries(current_user: UserModel, db: DB) -> APIResponse {
    use schema::libraries::dsl::*;
    let libs = libraries.load::<Library>(&*db).unwrap();
    ok().data(json!(libs))
}

#[get("/all_the_things")]
pub fn all_the_things(current_user: UserModel, db: DB) -> APIResponse {
    use schema;
    let libs = current_user.accessible_libraries(&*db).unwrap();
    let books = current_user.accessible_audiobooks(&*db).unwrap();
    let chapters: Vec<Chapter> = books.clone().into_iter().flat_map(|b| Chapter::belonging_to(&b).load::<Chapter>(&*db).unwrap()).collect();
    ok().data(json!({
        "libraries": libs,
        "books": books,
        "chapters": chapters,
    }))
}
