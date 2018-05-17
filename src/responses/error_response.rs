use std::io::Cursor;
use failure::Error;
use rocket::Request;
use rocket::response::{Response, Responder};
use rocket::request::FromRequest;
use serde_json::error::Error as SerdeError;
use rocket::http::{Status, ContentType};

use config::Config;

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

impl From<SerdeError> for APIError {
    fn from(error: SerdeError) -> Self {
        APIError {
            message: Some(format!("Error parsing input: {}", error)),
            error: Some(Error::from(error)),
            status: Status::BadRequest
        }
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
