use std::path::{Path, PathBuf};
use super::muxer;
use super::mediafile::MediaFile;
use super::error::*;
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
    files.iter().map(|s| "test-data/".to_owned() + s.to_owned()).map(
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

#[test]
fn list_chapters() {
    let file = MediaFile::read_file(Path::new("test-data/all.m4b")).unwrap();
    let mut chapters = file.get_chapters();
    assert_eq!(chapters.len(), 4);
    assert_eq!(chapters[2].clone().title.unwrap(), "3 - Otpluva lekii cheln...");
    assert_eq!(chapters[2].clone().start.floor() as usize, 91);
    println!("{:?}", chapters);
}

#[test]
#[should_panic(expected="No such file or directory")]
fn file_not_existing() {
    let f = MediaFile::read_file(
        Path::new("ifyoucreatedthisyouonlyhaveyourselftoblame.mp3")
        );
    match f {
        Err(me) => assert!(me.description.starts_with("No such file")),
        Ok(_) => panic!("We expect a Media Error here.")
    }
    let file = MediaFile::read_file(
        Path::new("ifyoucreatedthisyouonlyhaveyourselftoblame.mp3")
    ).unwrap();
}

#[test]
fn get_thumbnail() {
    let f = MediaFile::read_file(Path::new("test-data/1.mp3")).unwrap();
    let data = f.get_cover_art().unwrap().unwrap();
    // Check for jpeg header
    assert_eq!(255, data[0]);
    assert_eq!(216, data[1]);
}
