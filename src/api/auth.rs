use rocket_contrib::Json;
use validation::user::UserSerializer;
use diesel::prelude::*;
use diesel;

use models::user::{UserModel, NewUser};
use schema::users;
use schema::users::dsl::*;
use helpers::db::DB;
use responses::{APIResponse, ok, created, conflict, unauthorized, internal_server_error};
use rocket::Outcome;
use rocket::http::Status;
use validation::token::TokenSerializer;

#[post("/login", data = "<user_in>", format = "application/json")]
pub fn login(user_in: Json<UserSerializer>, db: DB) -> APIResponse {
    let results = users.filter(email.eq(user_in.email.clone()))
        .first::<UserModel>(&*db);

    if results.is_err() {
        return unauthorized().message("Username or password incorrect.");
    }

    let user = results.unwrap();
    if !user.verify_password(user_in.password.as_str()) {
        return unauthorized().message("Username or password incorrect.");
    }

    let token = match user.generate_api_token(db) {
        Ok(token) => token,
        _ => return internal_server_error()
    };

    ok().data(json!(
        TokenSerializer::from(token)
    ))
}

#[post("/register", data = "<user>", format = "application/json")]
pub fn register(user: Json<UserSerializer>, db: DB) -> Result<APIResponse, APIResponse> {
    let new_user = UserModel::create(&user.email, &user.password, &*db)?;

    Ok(created().message("User created.").data(json!(&new_user)))
}


#[get("/whoami")]
pub fn whoami(current_user: UserModel) -> APIResponse {
    ok().data(json!(&current_user))
}
