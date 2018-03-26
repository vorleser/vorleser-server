use rocket::response::{Responder, Result, Response};
use std::io::Cursor;
use rocket::http::ContentType;
use rocket::Request;

pub struct StaticFile(&'static [u8], ContentType);

impl StaticFile {
    fn new(data: &'static [u8], content_type: ContentType) -> StaticFile {
        StaticFile(data, content_type)
    }
}

impl<'r> Responder<'r> for StaticFile {
    fn respond_to(self, req: &Request) -> Result<'r> {
        let mut response = Response::build()
            .header(ContentType::Plain)
            .sized_body(Cursor::new(self.0))
            .ok()?;
        response.set_header(self.1);
        Ok(response)
    }
}

// TODO: it is probably possible to replace this verbose solution with a somewhat nice procedural
// macro. This macro would be used to generate routes.
// It should look something like this:
// `static_routes!["files/index.html"]` this macro would be used instead of the normal routes
// macro in the mount call in our rocket factory.
// What remains to be seen is how we would go about mounting index.html at "/" using this method.
// Maybe we would just mount it as before, or we could optionally pass tuples with a target path to
// the macro. Another issue would be the one of figuring out base paths, for files that are in
// subdirectories.

#[get("/")]
pub fn get_index() -> StaticFile {
    StaticFile(include_bytes!("../vorleser-web/index.html"), ContentType::from_extension("html").unwrap())
}

#[get("/elm.js")]
pub fn get_elmjs() -> StaticFile {
    StaticFile(include_bytes!("../vorleser-web/elm.js"), ContentType::from_extension("js").unwrap())
}

#[get("/session.js")]
pub fn get_sessionjs() -> StaticFile {
    StaticFile(include_bytes!("../vorleser-web/session.js"), ContentType::from_extension("js").unwrap())
}

#[get("/audio.js")]
pub fn get_audiojs() -> StaticFile {
    StaticFile(include_bytes!("../vorleser-web/audio.js"), ContentType::from_extension("js").unwrap())
}

#[get("/app.css")]
pub fn get_appcss() -> StaticFile {
    StaticFile(include_bytes!("../vorleser-web/app.css"), ContentType::from_extension("css").unwrap())
}

#[get("/vendor/roboto.css")]
pub fn get_robotocss() -> StaticFile {
    StaticFile(include_bytes!("../vorleser-web/vendor/roboto.css"), ContentType::from_extension("css").unwrap())
}

#[get("/vendor/material.min.css")]
pub fn get_materialcss() -> StaticFile {
    StaticFile(include_bytes!("../vorleser-web/vendor/material.min.css"), ContentType::from_extension("css").unwrap())
}
