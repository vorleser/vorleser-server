use std::fmt;
use std::ffi::CStr;
use ffmpeg::av_strerror;
use std::os::raw::c_char;
use walkdir;
use diesel;
use std::io;
use std::error::Error as StdError;
use std::str;
use std;

error_chain! {
    foreign_links {
        WalkDir(walkdir::Error);
        Db(diesel::result::Error);
        Io(io::Error);
        Utf8Error(::std::str::Utf8Error);
        Fmt(::std::fmt::Error);
    }

    errors {
        InvalidUtf8 {
            description("Invalid Utf-8")
        }
        Other(t: &'static str) {
            description(t)
        }

        MediaError(code: i32) {
            description(unsafe {
                println!("Error Code: {}", code);
                let mut buf: [c_char; 1024] = [0; 1024];
                av_strerror(*code, &mut buf[0], 1024);
                CStr::from_ptr(&buf[0]).to_str().unwrap()
            })
        }
    }
}
