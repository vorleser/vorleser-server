use crate::models::user::User;
use crate::responses::{APIResponse, ok};
use rocket_contrib::json::Json;
use diesel::prelude::*;
use diesel::BelongingToDsl;
use serde_json;
use crate::helpers::db::DB;
use crate::models::library::Library;
use crate::models::audiobook::Audiobook;
use crate::models::chapter::Chapter;
use crate::models::playstate::{Playstate, ApiPlaystate};

#[get("/libraries")]
pub fn libraries(current_user: User, db: DB) -> APIResponse {
    use crate::schema::libraries::dsl::*;
    let libs = libraries.load::<Library>(&*db).unwrap();
    ok().data(json!(libs))
}

#[get("/all_the_things")]
pub fn all_the_things(current_user: User, db: DB) -> APIResponse {
    use crate::schema;
    let libs = current_user.accessible_libraries(&*db).unwrap();
    let books = current_user.accessible_audiobooks(&*db).unwrap();
    let chapters: Vec<Chapter> = books.clone().into_iter().flat_map(|b| Chapter::belonging_to(&b).load::<Chapter>(&*db).unwrap()).collect();
    let playstates: Vec<_> = Playstate::belonging_to(&current_user).load::<Playstate>(&*db)
                                .unwrap().into_iter().map(|p| p.to_api_playstate()).collect();
    ok().data(json!({
        "libraries": libs,
        "books": books,
        "chapters": chapters,
        "playstates": playstates,
    }))
}

#[post("/update_playstates", data = "<playstate>", format = "application/json")]
pub fn update_playstates(playstate: Json<Vec<ApiPlaystate>>, current_user: User, db: DB) -> APIResponse {
    use diesel;
    // TODO: Don't ignore errors here
    db.exclusive_transaction(|| -> Result<(), diesel::result::Error> {
        for state in playstate.into_inner() {
            state.to_playstate(&current_user)
                .upsert(&*db)?.to_api_playstate();
        }
        Ok(())
    });
    ok().data(json!({}))
}
