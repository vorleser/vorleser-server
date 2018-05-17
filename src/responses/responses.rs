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

pub fn method_not_allowed() -> APIResponse {
    APIResponse {
        message: Some("Method Not Allowed".to_string()),
        data: None,
        status: Status::MethodNotAllowed,
    }
}

pub fn conflict() -> APIResponse {
    APIResponse {
        message: Some("Conflict".to_string()),
        data: None,
        status: Status::Conflict,
    }
}

pub fn unprocessable_entity() -> APIResponse {
    APIResponse {
        message: Some("Unprocessable Entity".to_string()),
        data: None,
        status: Status::UnprocessableEntity,
    }
}

pub fn internal_server_error() -> APIError {
    APIError::new(Status::InternalServerError).message("Internal Server Error")
}

pub fn service_unavailable() -> APIResponse {
    APIResponse {
        message: Some("Service Unavailable".to_string()),
        data: None,
        status: Status::ServiceUnavailable,
    }
}
