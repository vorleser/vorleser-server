use std::path::{Path, PathBuf};
use super::muxer;
use super::mediafile::MediaFile;
use super::mediafile::ImageType;
use std::env;
use std::io::Cursor;
use std::fs::create_dir_all;
use image::jpeg::JPEGDecoder;
use image::png::PNGDecoder;
use image::ImageDecoder;
use ::helpers::db::init_db_pool;

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
    let chapters = file.get_chapters();
    assert_eq!(chapters.len(), 4);
    assert_eq!(chapters[2].clone().title.unwrap(), "3 - Otpluva lekii cheln...");
    assert_eq!(chapters[2].clone().start.floor() as usize, 91);
    println!("{:?}", chapters);
}

#[test]
fn media_length() {
    let file = MediaFile::read_file(Path::new("test-data/all.m4b")).unwrap();
    assert_eq!(file.get_mediainfo().length.floor() as usize,  165);
}

#[test]
fn media_title() {
    let file = MediaFile::read_file(Path::new("test-data/all.m4b")).unwrap();
    let mi = file.get_mediainfo();
    assert_eq!("[Bulgarian]Stihotvorenia", mi.title)
}

#[test]
fn media_metadata() {
    let file = MediaFile::read_file(Path::new("test-data/all.m4b")).unwrap();
    assert_eq!(file.get_mediainfo().metadata.get("artist").unwrap(), "Mara Belcheva");
}

#[test]
fn media_no_metadata() {
    let file = MediaFile::read_file(Path::new("test-data/no_metadata.mp3")).unwrap();
    assert_eq!(file.get_mediainfo().title, "no_metadata.mp3");
}

#[test]
fn file_not_existing() {
    let f = MediaFile::read_file(
        Path::new("ifyoucreatedthisyouonlyhaveyourselftoblame.mp3")
        );
    match f {
        Err(me) => assert!(me.description.starts_with("No such file")),
        Ok(_) => panic!("We expect a Media Error here.")
    }
}

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
fn create_audiobook() {
    use super::scanner;
    let pool = init_db_pool();
    scanner::create_audiobook(pool.get().unwrap(), Path::new("test-data/all.m4b")).unwrap();
}

#[test]
fn checksum() {
    use super::scanner;
    let checksum = scanner::checksum_file(Path::new("test-data/all.m4b"));
    assert_slice_starts_with(&checksum.unwrap(), &[0x48, 0xab, 0x4a])
}

fn assert_slice_starts_with(bytes: &[u8], start: &[u8]) {
    let mut i = bytes.iter();
    for b in start {
        assert_eq!(i.next().unwrap(), b);
    }
}
