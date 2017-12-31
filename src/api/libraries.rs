use models::user::User;
use responses::{APIResponse, ok};
use rocket_contrib::Json;
use diesel::prelude::*;
use diesel::BelongingToDsl;
use serde_json;
use helpers::db::DB;
use models::library::Library;
use models::audiobook::Audiobook;
use models::chapter::Chapter;
use models::playstate::{Playstate, ApiPlaystate};

#[get("/libraries")]
pub fn libraries(current_user: User, db: DB) -> APIResponse {
    use schema::libraries::dsl::*;
    let libs = libraries.load::<Library>(&*db).unwrap();
    ok().data(json!(libs))
}

#[get("/all_the_things")]
pub fn all_the_things(current_user: User, db: DB) -> APIResponse {
    use schema;
    let libs = current_user.accessible_libraries(&*db).unwrap();
    let books = current_user.accessible_audiobooks(&*db).unwrap();
    let chapters: Vec<Chapter> = books.clone().into_iter().flat_map(|b| Chapter::belonging_to(&b).load::<Chapter>(&*db).unwrap()).collect();
    let playstates: Vec<_> = Playstate::belonging_to(&current_user).load::<Playstate>(&*db)
                                .unwrap().into_iter().map(|p| p.into_api_playstate()).collect();
    ok().data(json!({
        "libraries": libs,
        "books": books,
        "chapters": chapters,
        "playstates": playstates,
    }))
}

#[post("/update_playstates", data = "<playstate>", format = "application/json")]
pub fn update_playstates(playstate: Json<Vec<ApiPlaystate>>, current_user: User, db: DB) -> APIResponse {
    for state in playstate.into_inner() {
        state.into_playstate(&current_user).upsert(&*db).unwrap().into_api_playstate();
    }
    ok().data(json!({}))
}
