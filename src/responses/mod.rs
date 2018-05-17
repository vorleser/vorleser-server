pub mod responses;
pub mod error_response;
pub mod success_response;
pub use self::responses::*;
pub use self::success_response::APIResponse;
pub use self::error_response::APIError;

pub type APIResult = Result<APIResponse, APIError>;
