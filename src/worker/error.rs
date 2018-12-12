use std::fmt;
use std::ffi::CStr;
use crate::ffmpeg::av_strerror;
use std::os::raw::c_char;
use walkdir;
use diesel;
use crate::helpers;
use std::io;
use std::error::Error as StdError;
use std::result::Result as StdResult;
use failure::Error;
use std::str;
use std;

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Fail)]
pub enum WorkerError {
    #[fail(display = "Invalid Utf-8")]
    InvalidUtf8,
    #[fail(display = "{}", description)]
    Other {
        description: String
    },
    #[fail(display = "Can't aquire lockfile, is there another scan runnning?")]
    Locked,
    #[fail(display = "Error {}: {}", code, description)]
    MediaError {
        description: String,
        code: i32,
    },
    #[fail(display = "No format could be guessed")]
    UnkownFormat,
    #[fail(display = "No valid file extensions were found.")]
    NoValidFileExtensions,
    #[fail(display = "This file is not an audio file")]
    NotAnAudioFile,
    #[fail(display = "This path is outside the library")]
    OutsideLibrary,
}

pub fn new_media_error(code: i32) -> WorkerError {
    unsafe {
        let mut buf: [c_char; 1024] = [0; 1024];
        av_strerror(code, &mut buf[0], 1024);
        let cmsg = CStr::from_ptr(&buf[0]).to_str().unwrap();
        let mut msg = String::new();
        msg += cmsg;
        WorkerError::MediaError{
            description: msg,
            code,
        }
    }
}
