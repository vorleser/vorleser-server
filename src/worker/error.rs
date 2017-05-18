use std::fmt;
use std::ffi::CStr;
use ffmpeg::av_strerror;
use std::os::raw::c_char;
use walkdir;
use diesel;
use std::io;
use std::error::Error as StdError;

error_chain! {
    foreign_links {
        Media(MediaError);
        WalkDir(walkdir::Error);
        Db(diesel::result::Error);
        Io(io::Error);
    }

    errors {
        InvalidUtf8 {
            description("Invalid Utf-8")
        }
        Other(t: &'static str) {
            description(t)
        }
        MediaError {
        }
    }
}

#[derive(Debug)]
pub struct MediaError {
    pub code: i32,
    pub description: String
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl MediaError {
    pub fn new(code: i32) -> MediaError {
        let description = unsafe {
            let mut buf: [c_char; 1024] = [0; 1024];
            av_strerror(code, &mut buf[0], 1024);
            CStr::from_ptr(&buf[0]).to_string_lossy().into_owned()
        };
        MediaError {code: code, description: description}
    }

    pub fn from_description(code: i32, description: String) -> MediaError {
        MediaError{code: code, description: description}
    }
}

impl StdError for MediaError {
    fn description(&self) -> &str {
        return &self.description;
    }

    fn cause(&self) -> Option<&StdError> {
        None
    }
}
