use super::success_response::APIResponse;
use super::error_response::APIError;
use rocket::http::Status;

pub fn ok() -> APIResponse {
    APIResponse {
        message: Some("Ok".to_string()),
        data: None,
        status: Status::Ok,
    }
}

pub fn created() -> APIResponse {
    APIResponse {
        message: Some("Created".to_string()),
        data: None,
        status: Status::Created,
    }
}

pub fn accepted() -> APIResponse {
    APIResponse {
        message: Some("Accepted".to_string()),
        data: None,
        status: Status::Accepted,
    }
}

pub fn no_content() -> APIError {
    APIError::new(Status::NoContent).message("No Content")
}


pub fn bad_request() -> APIError {
    APIError::new(Status::BadRequest).message("Bad Request")
}

pub fn unauthorized() -> APIError {
    APIError::new(Status::Unauthorized)
        .message("Unauthorized")
}

pub fn forbidden() -> APIError {
    APIError::new(Status::Forbidden).message("Forbidden")
}

pub fn not_found() -> APIError {
    APIError::new(Status::NotFound).message("Not Found")
}

pub fn method_not_allowed() -> APIError {
    APIError::new(Status::MethodNotAllowed).message("Method Not Allowed")
}

pub fn conflict() -> APIError {
    APIError::new(Status::Conflict).message("Conflict")
}

pub fn unprocessable_entity() -> APIError {
    APIError::new(Status::UnprocessableEntity).message("Unprocessable Entity")
}

pub fn internal_server_error() -> APIError {
    APIError::new(Status::InternalServerError).message("Internal Server Error")
}

pub fn service_unavailable() -> APIError {
    APIError::new(Status::ServiceUnavailable).message("Service Unavailable")
}
