use rocket_contrib::Json;
use validation::user::UserSerializer;
use diesel::prelude::*;
use diesel;

use models::user::{User, NewUser};
use schema::users;
use schema::users::dsl::*;
use helpers::db::DB;
use responses::{APIResponse, ok, created, conflict, unauthorized, internal_server_error};
use rocket::Outcome;
use rocket::http::Status;
use validation::token::TokenSerializer;

#[post("/login", data = "<user_in>", format = "application/json")]
pub fn login(user_in: Json<UserSerializer>, db: DB) -> Result<APIResponse, APIResponse>  {
    let results = users.filter(email.eq(user_in.email.clone()))
        .first::<User>(&*db);

    if results.is_err() {
        return Ok(unauthorized().message("Username or password incorrect."));
    }

    let user = results.unwrap();
    if !user.verify_password(user_in.password.as_str()) {
        return Ok(unauthorized().message("Username or password incorrect."));
    }

    let token = user.generate_api_token(db)?;

    Ok(ok().data(json!(
        TokenSerializer::from(token)
    )))
}

#[post("/register", data = "<user>", format = "application/json")]
pub fn register(user: Json<UserSerializer>, db: DB) -> Result<APIResponse, APIResponse> {
    let new_user = User::create(&user.email, &user.password, &*db)?;

    Ok(created().message("User created.").data(json!(&new_user)))
}


#[get("/whoami")]
pub fn whoami(current_user: User) -> APIResponse {
    ok().data(json!(&current_user))
}
