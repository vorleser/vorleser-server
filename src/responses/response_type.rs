use std::io::Cursor;
use rocket_contrib::json::{JsonValue, Json};
use rocket::request::Request;
use rocket::response::{Response, Responder};
use rocket::http::{Status, ContentType};
use super::responses::*;
use diesel;
use uuid;
use models::user::Error as UserModelError;
use models::user::ErrorKind as UserModelErrorKind;

#[derive(Debug)]
pub struct APIResponse {
    pub(super) message: Option<String>,
    pub(super) data: Option<JsonValue>,
    pub(super) status: Status,
}

impl APIResponse {
    /// Change the message of the `Response`.
    pub fn message(mut self, message: &str) -> APIResponse {
        self.message = Some(message.to_string());
        self
    }

    /// Change the data to the `Response`.
    pub fn data(mut self, data: JsonValue) -> APIResponse {
        self.data = Some(data);
        self
    }
}

impl<'r> Responder<'r> for APIResponse {
    fn respond_to(self, request: &Request) -> Result<Response<'r>, Status> {
        let body = match (self.data, self.message) {
            (Some(data), _) => data,
            (_, Some(message)) => json!({ "message": message }),
            (None, None) => panic!()
        };

        Response::build()
            .status(self.status)
            .sized_body(Cursor::new(body.to_string()))
            .header(ContentType::JSON)
            .ok()
    }
}

impl From<uuid::ParseError> for APIResponse {
    fn from(error: uuid::ParseError) -> Self {
        bad_request()
    }
}

impl From<UserModelError> for APIResponse {
    fn from(error: UserModelError) -> Self {
        match error.kind() {
            &UserModelErrorKind::UserExists(ref user_name) =>
                conflict().message(&format!("{}", error)),
            &UserModelErrorKind::Db(ref db_error) => APIResponse::from(db_error),
            _ => bad_request().message("Something is wrong with the auth token or login details you provided.")
        }
    }
}

impl From<diesel::result::Error> for APIResponse {
    fn from(error: diesel::result::Error) -> Self {
        APIResponse::from(&error)
    }
}

impl<'a> From<&'a diesel::result::Error> for APIResponse {
    fn from(error: &diesel::result::Error) -> Self {
        use diesel::result::Error;
        match error {
            &Error::NotFound => not_found(),
            _ => internal_server_error()
        }
    }
}
