use rocket::Outcome;
use rocket::http::Status;
use rocket::request::{self, Request, FromRequest};

use models::user::{self, User, ApiToken};
use models::library::Library;
use diesel;
use diesel::prelude::*;
use helpers::uuid::Uuid;
use helpers::db::DB;
use responses::{APIResponse, APIError, bad_request, unauthorized, forbidden, not_found,
                internal_server_error, service_unavailable};



impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<User, ()> {
        use schema::users::dsl;

        let token_result = <ApiToken as FromRequest>::from_request(request);
        let db = <DB as FromRequest>::from_request(request).unwrap();
        println!("{:?}", token_result);
        match token_result {
            Outcome::Success(token) => Outcome::Success(
                dsl::users.filter(dsl::id.eq(token.user_id))
                    .first::<User>(&*db)
                    .unwrap()),
            Outcome::Failure(err) => Outcome::Failure(err),
            Outcome::Forward(f) => Outcome::Forward(f)
        }
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for ApiToken {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<ApiToken, ()> {
        let db = <DB as FromRequest>::from_request(request).unwrap();
        let mut tokens = request.headers().get("Authorization");
        let token = match tokens.next() {
            Some(t) => t,
            None => {
                match request.uri().query().and_then(|q| {
                    q.split('&')
                     .filter(|s| s.starts_with("auth="))
                     .map(|s| s.split_at(5).1)
                     .next()
                }) {
                    Some(t) => t,
                    None => return Outcome::Failure((Status::Unauthorized, ()))
                }
            }
        };
        use schema;
        use schema::api_tokens::dsl::api_tokens;

        use schema::api_tokens::dsl::id as token_id;
        use diesel::query_dsl::filter_dsl::FilterDsl;
        use diesel::RunQueryDsl;
        use diesel::prelude::*;

        if let Ok(submitted_id) = Uuid::parse_str(token) {
            let token_option = diesel::query_dsl::filter_dsl::
                FilterDsl::filter(api_tokens, token_id.eq(&submitted_id))
                .first::<ApiToken>(&*db)
                .optional()
                .expect("Database error!");
            match token_option {
                Some(token) => Outcome::Success(token),
                None => Outcome::Failure((Status::Unauthorized, ()))
            }
        } else {
            Outcome::Failure((Status::BadRequest, ()))
        }
    }
}

#[catch(400)]
fn bad_request_handler() -> APIError {
    bad_request()
}

#[catch(401)]
fn unauthorized_handler() -> APIError {
    unauthorized()
}

#[catch(403)]
fn forbidden_handler() -> APIError {
    forbidden()
}

#[catch(404)]
fn not_found_handler() -> APIError {
    not_found()
}

#[catch(500)]
fn internal_server_error_handler() -> APIError {
    internal_server_error()
}

#[catch(503)]
fn service_unavailable_handler() -> APIError {
    service_unavailable()
}
