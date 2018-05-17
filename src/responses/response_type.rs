use std::io::Cursor;
use rocket_contrib::json::{Json};
use rocket_contrib::Value;
use rocket::request::{Request, FromRequest};
use rocket::response::{Response, Responder};
use rocket::http::{Status, ContentType};
use super::responses::*;
use config::Config;
use diesel;
use uuid;
use models::user::UserError;
use failure::Error;

#[derive(Debug)]
pub struct APIResponse {
    pub(super) message: Option<String>,
    pub(super) data: Option<Value>,
    pub(super) status: Status,
}

#[derive(Debug)]
pub struct APIError {
    pub(super) message: Option<String>,
    pub(super) error: Option<Error>,
    pub(super) status: Status,
}

impl APIError {
    pub fn builder(status: Status) -> APIErrorBuilder {
        APIErrorBuilder::new(status)
    }
}

pub struct APIErrorBuilder(APIError);

impl APIErrorBuilder {
    pub fn new(status: Status) -> Self {
        APIErrorBuilder(APIError {
            message: None,
            error: None,
            status: status,
        })
    }

    pub fn message(mut self, msg: String) -> Self {
        self.0.message = Some(msg);
        self
    }

    pub fn error(mut self, err: Error) -> Self {
        self.0.error = Some(err);
        self
    }

    pub fn build(self) -> APIError {
        self.0
    }
}

impl From<Error> for APIError {
    fn from(error: Error) -> Self {
        APIError {
            message: None,
            error: Some(error),
            status: Status::InternalServerError
        }
    }
}

impl<'r> Responder<'r> for APIError {
    fn respond_to(self, request: &Request) -> Result<Response<'r>, Status> {
        let debug = true;

        // TODO: use debug from config
        let config = Config::from_request(request);
        let body = match (debug, self.message, &self.error.as_ref()) {
            (false, Some(msg), _) => json!({"message": msg}),
            (false, None, _) => json!({}),
            (true, None, err) => json!({
                "error": err.map(|err| err.to_string()),
                "backtrace": err.map(|err|
                    format!("{}", err.backtrace())
                ),
            }),
            (true, Some(msg), err) => json!({
                "message": msg,
                "error": err.map(|err| err.to_string()),
                "backtrace": err.map(|err|
                    format!("{}", err.backtrace())
                ),
            })
        };

        Response::build()
            .status(self.status)
            .sized_body(Cursor::new(body.to_string()))
            .header(ContentType::JSON)
            .ok()
    }
}

impl APIResponse {
    /// Change the message of the `Response`.
    pub fn message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }

    /// Change the data to the `Response`.
    pub fn data(mut self, data: Value) -> Self {
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

impl From<Error> for APIResponse {
    fn from(error: Error) -> Self {
        if let Some(err) = error.downcast_ref::<UserError>() {
            return err.into()
        }
        if let Some(err) = error.downcast_ref::<diesel::result::Error>() {
            return err.into()
        }
        return internal_server_error()
    }
}

impl<'a> From<&'a UserError> for APIResponse {
    fn from(error: &UserError) -> Self {
        match error {
            &UserError::AlreadyExists{ ref user_name } =>
                conflict().message(&format!("{}", user_name))
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
