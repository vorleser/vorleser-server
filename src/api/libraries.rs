use models::user::UserModel;
use responses::{APIResponse, ok};
use rocket_contrib::JSON;
use diesel::prelude::*;
use serde_json;
use helpers::db::DB;
use models::library::Library;

#[get("/libraries")]
pub fn libraries(current_user: UserModel, db: DB) -> APIResponse {
    use schema::libraries::dsl::*;
    let libs = libraries.load::<Library>(&*db).unwrap();
    json!(libs)
}
