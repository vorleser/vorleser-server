use rocket_contrib::Json;
use validation::user::UserSerializer;
use diesel::prelude::*;
use diesel;
use failure::Error;
use serde_json::error::Error as SerdeError;

use config::Config;
use responses;
use models::user::{User, NewUser, ApiToken};
use schema::users;
use schema::users::dsl::*;
use helpers::db::DB;
use responses::{APIError, APIResponse, APIResult, ok, created, conflict, unauthorized, internal_server_error};
use rocket::http::Status;
use validation::token::TokenSerializer;
use helpers::JsonResult;

#[post("/login", data = "<user_in>", format = "application/json")]
pub fn login(user_in: Json<UserSerializer>, db: DB) -> Result<APIResponse, APIError> {
    let results = users.filter(email.eq(user_in.email.clone()))
        .first::<User>(&*db);

    if results.is_err() {
        return Err(unauthorized().message("Username or password incorrect."));
    }

    let user = results.unwrap();
    if !user.verify_password(user_in.password.as_str()) {
        return Err(unauthorized().message("Username or password incorrect."));
    }

    let token = user.generate_api_token(db)?;

    Ok(ok().data(json!(
        TokenSerializer::from(token)
    )))
}

#[post("/register", data = "<user>", format = "application/json")]
pub fn register(user: Json<UserSerializer>, db: DB, config: Config) -> APIResult {
    if config.register_web {
        let new_user = User::create(&user.email, &user.password, &*db)?;
        Ok(created().message("User created.").data(json!(&new_user)))
    } else {
        Err(responses::unauthorized().message("Registration is disabled. Create a user via the commandline or enable user \
                                               creation in the config file."))
    }
}


#[get("/whoami")]
pub fn whoami(current_user: User) -> APIResponse {
    ok().data(json!(&current_user))
}

#[post("/logout")]
pub fn logout(current_user: User, token: ApiToken, db: DB) -> Result<APIResponse, APIError> {
    use schema::api_tokens::table;
    use schema::api_tokens::dsl::id;

    let ret = diesel::delete(table.filter(id.eq(token.id))).execute(&*db)?;
    println!("{}", ret);
    Ok(ok())
}

#[post("/logout_all")]
pub fn logout_all(current_user: User, token: ApiToken, db: DB) -> Result<APIResponse, APIError> {
    use schema::api_tokens::table;
    use schema::api_tokens::dsl::user_id;

    diesel::delete(table.filter(user_id.eq(current_user.id))).execute(&*db)?;
    Ok(ok())
}
