use std::io::Cursor;
use failure::Error;
use rocket::Request;
use rocket::Outcome;
use rocket::response::{Response, Responder};
use rocket::request::FromRequest;
use rocket::http::{Status, ContentType};
use models::user::UserError;
use uuid;
use responses::responses::{bad_request, not_found, internal_server_error, conflict};
use serde_json::error::Error as SerdeError;
use diesel;

use config::Config;

#[derive(Debug)]
pub struct APIError {
    pub(super) message: Option<String>,
    pub(super) error: Option<Error>,
    pub(super) status: Status,
}

impl APIError {
    pub fn new(status: Status) -> Self {
        Self {
            message: None,
            error: None,
            status: status,
        }
    }

    pub fn message(mut self, msg: &str) -> Self {
        self.message = Some(msg.to_owned());
        self
    }

    pub fn error(mut self, err: Error) -> Self {
        self.error = Some(err);
        self
    }
}

impl From<uuid::ParseError> for APIError {
    fn from(error: uuid::ParseError) -> Self {
        bad_request()
    }
}


impl From<SerdeError> for APIError {
    fn from(error: SerdeError) -> Self {
        APIError {
            message: Some(format!("Error parsing input: {}", error)),
            error: Some(Error::from(error)),
            status: Status::BadRequest
        }
    }
}

fn backtrace_list(err: &Error) -> Vec<String> {
    format!("{}", err.backtrace())
        .lines()
        .map(|s| s.to_owned())
        .collect::<Vec<String>>()
}

impl<'r> Responder<'r> for APIError {
    fn respond_to(self, request: &Request) -> Result<Response<'r>, Status> {
        let config_outcome = Config::from_request(request);
        let debug = match config_outcome {
            Outcome::Success(conf) => conf.web.debug,
            _ => false,
        };

        let body = match (debug, self.message, &self.error.as_ref()) {
            (false, Some(msg), _) => json!({"message": msg}),
            (false, None, _) => json!({}),
            (true, None, err) => json!({
                "error": err.map(|err| err.to_string()),
                "backtrace": err.map(backtrace_list),
            }),
            (true, Some(msg), err) => json!({
                "message": msg,
                "error": err.map(|err| err.to_string()),
                "backtrace": err.map(backtrace_list),
            })
        };

        Response::build()
            .status(self.status)
            .sized_body(Cursor::new(body.to_string()))
            .header(ContentType::JSON)
            .ok()
    }
}

impl From<diesel::result::Error> for APIError {
    fn from(error: diesel::result::Error) -> Self {
        APIError::from(&error)
    }
}

impl<'a> From<&'a diesel::result::Error> for APIError {
    fn from(error: &diesel::result::Error) -> Self {
        use diesel::result::Error;
        match error {
            &Error::NotFound => not_found(),
            _ => internal_server_error()
        }
    }
}

impl From<Error> for APIError {
    fn from(error: Error) -> Self {
        if let Some(err) = error.downcast_ref::<UserError>() {
            match err {
                &UserError::AlreadyExists {user_name: ref name} => {
                    let mut conflict_resp = conflict();
                    conflict_resp.message = Some(format!("The {} already exists", name));
                    return conflict_resp;
                }
            }
        }
        if let Some(err) = error.downcast_ref::<diesel::result::Error>() {
            return err.into()
        }
        APIError {
            message: None,
            error: Some(error),
            status: Status::InternalServerError
        }
    }
}

