extern crate vorleser_lib;

use std::path::{Path, PathBuf};
use vorleser_lib::worker::muxer;
use vorleser_lib::worker::mediafile::MediaFile;
use vorleser_lib::worker::error::*;
use std::env;
use std::fs::create_dir_all;

fn get_tempdir() -> PathBuf {
    let mut dir = env::temp_dir();
    dir.push("vorleser-tests");
    create_dir_all(&dir).unwrap();
    dir
}

fn read_files() -> Vec<MediaFile> {
    let files = vec!["1.mp3", "2.mp3", "3.mp3", "4.mp3"];
    files.iter().map(|s| "tests/media/".to_owned() + s.to_owned()).map(
        |name| MediaFile::read_file(Path::new(
            &name
        )).unwrap()
    ).collect()
}

#[test]
fn read_files_test() {
    let files = read_files();
    for f in files {
        assert_eq!(f.get_chapters().len(), 0)
    }
}

#[test]
fn concat_files() {
    let files = read_files();
    let mut tmp_dir = get_tempdir();
    tmp_dir.push(Path::new("muxed.mp3"));
    muxer::merge_files(&tmp_dir, files).unwrap();
}
