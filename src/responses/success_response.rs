use std::io::Cursor;
use rocket_contrib::json::{Json};
use rocket_contrib::json::JsonValue;
use rocket::request::{Request, FromRequest};
use rocket::response::{Response, Responder};
use rocket::http::{Status, ContentType};
use super::responses::*;
use config::Config;
use models::user::UserError;
use failure::Error;
use diesel;

#[derive(Debug)]
pub struct APIResponse {
    pub(super) message: Option<String>,
    pub(super) data: Option<JsonValue>,
    pub(super) status: Status,
}


impl APIResponse {
    /// Change the message of the `Response`.
    pub fn message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }

    /// Change the data to the `Response`.
    pub fn data(mut self, data: JsonValue) -> Self {
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
