use std::path::{Path, PathBuf};
use super::mediafile::MediaFile;
use std::env;
use std::fs::create_dir_all;
use super::muxer;
use std::io::Cursor;
use super::mediafile::ImageType;
use image::jpeg::JPEGDecoder;
use image::png::PNGDecoder;
use image::ImageDecoder;
use std::ffi::OsString;
use helpers;
use helpers::db::init_db_pool;
use diesel;
use diesel::prelude::*;
use ::worker::util;

describe! worker_tests {
    before_each {
        let mut pool = init_db_pool();
        let conn = pool.get().unwrap();
        util::shut_up_ffmpeg();
    }

    after_each {
        conn.execute("TRUNCATE audiobooks, chapters, playstates RESTART IDENTITY CASCADE").unwrap();
    }

    describe! scanner_tests {
        before_each {
            use models::library::{NewLibrary, Library};
            use schema::libraries;
            use worker::scanner;
            let new_lib = NewLibrary{
                location: "test-data".to_owned(),
                is_audiobook_regex: "^[^/]+$".to_owned()
            };
            let library: Library = diesel::insert(&new_lib)
                .into(libraries::table)
                .get_result(&*conn)
                .unwrap();
            let test_scanner = scanner::Scanner::new(init_db_pool(), library.clone());
        }

        it "Can create single file audiobooks" {
            use ::models::audiobook::{Audiobook, NewAudiobook, Update};
            test_scanner.create_audiobook(&*conn, &Path::new("test-data/all.m4b")).unwrap();
            assert_eq!(1, Audiobook::belonging_to(&library).count().first::<i64>(&*conn).unwrap());
        }

    }
}


describe! mediafile_tests {
    before_each {
        let file = MediaFile::read_file(Path::new("test-data/all.m4b")).unwrap();
        util::shut_up_ffmpeg();
    }

    it "can be probed" {
        file.probe_format();
    }

    it "handles non existing files" {
        let invalid_file = MediaFile::read_file(
            Path::new("ifyoucreatedthisyouonlyhaveyourselftoblame.mp3")
            );
        match invalid_file {
            Err(me) => {
                println!("{:?}", me.description());
                assert!(me.description().starts_with("No such file"));
            },
            Ok(_) => panic!("We expect a Media Error here.")
        }
    }

    it "reads chapters" {
        let chapters = file.get_chapters();
        assert_eq!(chapters.len(), 4);
        assert_eq!(chapters[2].clone().title.unwrap(), "3 - Otpluva lekii cheln...");
        assert_eq!(chapters[2].clone().start.floor() as usize, 91);
        println!("{:?}", chapters);
    }

    it "get's the length right" {
        assert_eq!(file.get_mediainfo().length.floor() as usize,  165);
    }

    it "reads the title" {
        let mi = file.get_mediainfo();
        assert_eq!("[Bulgarian]Stihotvorenia", mi.title)
    }

    it "has metadata" {
        let file = MediaFile::read_file(Path::new("test-data/all.m4b")).unwrap();
        assert_eq!(file.get_mediainfo().metadata.get("artist").unwrap(), "Mara Belcheva");
    }

    it "has defaults for file without metadata" {
        let file = MediaFile::read_file(Path::new("test-data/no_metadata.mp3")).unwrap();
        assert_eq!(file.get_mediainfo().title, "no_metadata.mp3");
    }

    describe! multi_files {
        before_each {
            let files = read_files();
        }

        it "doesn't see chapters where none are" {
            for f in files {
                assert_eq!(f.get_chapters().len(), 0)
            }
        }

        it "can remux files" {
            let mut tmp_dir = get_tempdir();
            tmp_dir.push(Path::new("muxed.mp3"));
            muxer::merge_files(&tmp_dir, &files).unwrap();
        }
    }
}

describe! mimetype {
    it "should find the mime type" {
        assert_eq!(util::sniff_mime_type(&"test-data/1.mp3".to_owned()).unwrap().unwrap(), "audio/mpeg")
    }
}

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
fn common_extension() {
    use worker::scanner::probable_audio_filetype;
    let ft = probable_audio_filetype(&"test-data/all");
    assert_eq!(ft.unwrap().unwrap().extension, OsString::from("mp3")) }

#[test]
fn get_thumbnail_jpg() {
    let j = MediaFile::read_file(Path::new("test-data/1.mp3")).unwrap();
    let jpeg_image = j.get_cover_art().unwrap().unwrap();
    assert_eq!(jpeg_image.image_type, ImageType::JPG);
    let mut jpeg_decoder = JPEGDecoder::new(Cursor::new(jpeg_image.data));
    let jpeg_dims = jpeg_decoder.dimensions().unwrap();
    assert_eq!((300, 300), jpeg_dims);
}

#[test]
fn get_thumbnail_png() {
    let f = MediaFile::read_file(Path::new("test-data/2.mp3")).unwrap();
    let png_image = f.get_cover_art().unwrap().unwrap();
    assert_eq!(png_image.image_type, ImageType::PNG);
    let mut png_decoder = PNGDecoder::new(Cursor::new(png_image.data));
    let png_dims = png_decoder.dimensions().unwrap();
    assert_eq!((300, 300), png_dims);
}

#[test]
fn checksum() {
    use super::scanner;
    let checksum = scanner::checksum_file(&Path::new("test-data/all.m4b"));
    assert_slice_starts_with(&checksum.unwrap(), &[0x48, 0xab, 0x4a])
}

#[test]
fn checksum_dir() {
    use super::scanner;
    let checksum = scanner::checksum_dir(&Path::new("test-data/all"));
    checksum.unwrap();
}

fn assert_slice_starts_with(bytes: &[u8], start: &[u8]) {
    let mut i = bytes.iter();
    for b in start {
        assert_eq!(i.next().unwrap(), b);
    }
}
