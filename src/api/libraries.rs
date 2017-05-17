use models::user::UserModel;
use responses::{APIResponse, ok};
use rocket_contrib::JSON;
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use models::library::Library;
use models::audiobook::Playstate;

#[get("/libraries")]
pub fn libraries(current_user: UserModel, db: DB) -> APIResponse {
    use schema::libraries::dsl::*;
    let libs = libraries.load::<Library>(&*db).unwrap();
    ok().data(json!(libs))
}

#[get("/all_the_things")]
pub fn all_the_things(current_user: UserModel, db: DB) -> APIResponse {
    ok().data(json!({ "some": "thing" }))
}

#[put("/update_playstate/<book_id>", format = "application/json", data = "<playstate>")]
pub fn update_playstate(current_user: UserModel, db: DB, book_id: &str, playstate: JSON<Playstate>) -> APIResponse {
    ok().data(json!({ "up": "dated" }))
}
