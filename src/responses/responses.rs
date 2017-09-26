use super::response_type::APIResponse;
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

pub fn no_content() -> APIResponse {
    APIResponse {
        message: Some("No Content".to_string()),
        data: None,
        status: Status::NoContent,
    }
}


pub fn bad_request() -> APIResponse {
    APIResponse {
        message: Some("Bad Request".to_string()),
        data: None,
        status: Status::BadRequest,
    }
}

pub fn unauthorized() -> APIResponse {
    APIResponse {
        message: Some("Unauthorized".to_string()),
        data: None,
        status: Status::Unauthorized,
    }
}

pub fn forbidden() -> APIResponse {
    APIResponse {
        message: Some("Forbidden".to_string()),
        data: None,
        status: Status::Forbidden,
    }
}

pub fn not_found() -> APIResponse {
    APIResponse {
        message: Some("Not Found".to_string()),
        data: None,
        status: Status::NotFound,
    }
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

pub fn internal_server_error() -> APIResponse {
    APIResponse {
        message: Some("Internal Server Error".to_string()),
        data: None,
        status: Status::InternalServerError,
    }
}

pub fn service_unavailable() -> APIResponse {
    APIResponse {
        message: Some("Service Unavailable".to_string()),
        data: None,
        status: Status::ServiceUnavailable,
    }
}
