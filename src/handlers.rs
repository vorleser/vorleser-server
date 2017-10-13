use rocket::Outcome;
use rocket::http::Status;
use rocket::request::{self, Request, FromRequest};

use models::user::{self, UserModel};
use models::library::Library;
use helpers::db::DB;
use responses::{APIResponse, bad_request, unauthorized, forbidden, not_found, internal_server_error,
                service_unavailable};



impl<'a, 'r> FromRequest<'a, 'r> for UserModel {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<UserModel, ()> {
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

        match UserModel::get_user_from_api_token(token, &*db) {
            Ok(Some(user)) => Outcome::Success(user),
            Err(user::Error(user::ErrorKind::Db(_), _)) => Outcome::Failure((Status::InternalServerError, ())),
            Err(user::Error(user::ErrorKind::UuidParse(_), _)) => Outcome::Failure((Status::BadRequest, ())),
            _ => Outcome::Failure((Status::Unauthorized, ())),
        }

    }
}
