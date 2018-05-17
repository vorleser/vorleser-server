use serde_json::error::Error as SerdeError;
use rocket_contrib::Json;

pub type JsonResult<T> = Result<Json<T>, SerdeError>;
