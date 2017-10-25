extern crate diesel;
use std::io;
use std::io::Read;
use std::fs::File;
use std::ffi::{OsString, OsStr};
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use walkdir::{WalkDir, WalkDirIterator};
use walkdir;
use regex::Regex;
use diesel::prelude::*;
use worker::mediafile::MediaFile;
use worker::error::*;
use ring::digest;
use humanesort::HumaneOrder;

use ::helpers::db::Pool;
use ::models::library::*;
use ::models::audiobook::{Audiobook, NewAudiobook, Update};
use ::models::chapter::{NewChapter, Chapter};
use ::schema::audiobooks;
use ::schema::chapters;
use ::schema::libraries;
use worker::muxer;
use chrono::prelude::*;
use chrono::NaiveDateTime;
use std::env;
use std::os::unix::prelude::*;
use std::os::unix::fs;
use std::fs::create_dir;
use diesel::BelongingToDsl;
use worker::util;
use worker::mediafile::Image;
use super::hashing;

pub struct Scanner {
    pub regex: Regex,
    pub library: Library,
    pub pool: Pool
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub(super) struct Filetype {
    pub extension: OsString,
    pub mime_type: String
}

enum Scan {
    Incremental,
    Full
}

/// Scanner object for scanning a library.
/// Construct this from a library object, then scan the library's directory using `scan_incremental`.
impl Scanner {
    pub fn new(conn_pool: Pool, library: Library) -> Self {
        Self {
            regex: Regex::new(library.is_audiobook_regex.as_str()).expect("Invalid Regex!"),
            library: library,
            pool: conn_pool
        }
    }

    /// Perform an incremental scan, this takes file change dates into account.
    /// As a result not all files are actually hashed. This should be the default behavior as it
    /// is much faster than hashing all files. If inconsistent situations arise a full scan might
    /// be able to fix the state, depending on what broke.
    pub fn incremental_scan(&mut self) -> Result<()> {
        self.scan_library(Scan::Incremental)
    }

    /// A full scan actually hashes each file that looks like an audiobook. This should only be run
    /// very sparingly. Maybe on specific user request or on a very long interval. This can easily
    /// keep the filesystem busy for a while if the library is sufficiently large.
    pub fn full_scan(&mut self) -> Result<()> {
        self.scan_library(Scan::Full)
    }

    // for all existing audiobooks
    // check hashes, if changed, remove book and create new with new data
    // if hashes have not changed: check symlinked/remuxed files still there? if not re-link/mux
    fn scan_library(&mut self, scan_type: Scan) -> Result<()> {
        info!("Scanning library: {}", self.library.location);
        let last_scan = self.library.last_scan;
        self.library.last_scan = Some(Utc::now().naive_utc());
        let conn = &*self.pool.get().unwrap();
        self.recover_deleted(conn);
        let mut walker = WalkDir::new(&self.library.location).follow_links(true).into_iter();

        loop {
            let entry = match walker.next() {
                None => break,
                Some(Err(e)) => return Err(e.into()),
                Some(Ok(i)) => i,
            };
            let path = entry.path();
            let relative_path = entry.path().strip_prefix(&self.library.location).unwrap();
            if relative_path.components().count() == 0 { continue };
            if is_audiobook(relative_path, &self.regex) {
                match scan_type {
                    Scan::Incremental => {
                        if should_scan(path, last_scan)? {
                            self.process_audiobook(&path, conn);
                        }
                    },
                    Scan::Full => self.process_audiobook(&path, conn)
                }
                // Since we are in an audiobook we don't continue searching deeper in the dir tree from here
                if path.is_dir() {
                    walker.skip_current_dir();
                }
            };

            ()
        };
        let deleted = self.delete_not_in_fs(conn)?;
        info!("Deleted {} audiobooks because their files are no longer present.", deleted);

        match diesel::update(libraries::dsl::libraries.filter(libraries::dsl::id.eq(self.library.id)))
            .set(&self.library)
            .execute(conn) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.into())
            }
    }

    fn process_audiobook(&self, path: &AsRef<Path>, conn: &PgConnection) {
        if path.as_ref().is_dir() {
            match self.create_multifile_audiobook(conn, path) {
                Ok(_) => (),
                Err(e) => error_log!("Error: {}", e.description())
            };
        } else {
            match self.create_audiobook(conn, path) {
                Ok(_) => (),
                Err(e) => error_log!("Error: {}", e.description())
            };
        }
    }


    /// Try to recover those books that were marked as deleted.
    /// Checks the file paths of books in the database and recovers them if hashes match
    fn recover_deleted(&self, conn: &PgConnection) {
        ()
    }

    /// Delete all those books from the database that are not present in the file system.
    fn delete_not_in_fs(&self, conn: &PgConnection) -> Result<usize> {
        // TODO: we should just mark books deleted here, after all accidents where the
        // filesystem is gone for a bit should not lead to you loosing all playback data
        // we should also be able to recover from having the book set to deleted
        let mut deleted = 0;
        for book in Audiobook::belonging_to(&self.library).get_results::<Audiobook>(&*conn)? {
            let path = Path::new(&self.library.location).join(Path::new(&book.location));
            info!("checking weather audiobook at {:?} still exists", path);
            if !path.exists() {
                let del = diesel::delete(
                        Audiobook::belonging_to(&self.library)
                        .filter(audiobooks::dsl::id.eq(book.id))
                    ).execute(&*conn)?;
                match del {
                    0 => warn!("Could not delete audiobook, is something wrong with the DB?"),
                    1 => deleted += 1,
                    x => {
                        warn!("Deleted multiple audiobooks with same UUID, database integrity is compromised.");
                        deleted += x;
                    }
                }
            }
        };
        Ok(deleted)
    }


    /// Audiobooks that are not remuxed are linked into our data directory so we have one canonical
    /// source of data.
    fn link_audiobook(&self, book: &Audiobook) -> Result<()> {
        let mut dest = PathBuf::from("data");
        dest.push(&book.id.hyphenated().to_string());
        dest.set_extension(&book.file_extension);
        let mut src = PathBuf::from(&self.library.location);
        src.push(&book.location);
        fs::symlink(src, dest);
        Ok(())
    }


    /// Save cover art to directory
    fn save_coverart(&self, book: &Audiobook, image: &Image) -> Result<()> {
        match create_dir("data/img") {
            Err(e) => match e.kind() {
                io::ErrorKind::AlreadyExists => (),
                _ => Err(e)?
            },
            Ok(_) => ()
        };
        let mut dest = PathBuf::from("data/img");
        dest.push(&book.id.hyphenated().to_string());
        image.save(&dest)?;
        Ok(())
    }

    pub(super) fn create_audiobook(&self, conn: &diesel::pg::PgConnection, path: &AsRef<Path>) -> Result<()> {
        let relative_path = self.relative_path_str(path)?;
        let hash = hashing::checksum_file(path)?;

        let done = match Audiobook::update_path(&hash, &relative_path, conn)? {
            Update::Nothing | Update::Path => true,
            Update::NotFound => false
        };
        if done {
            return Ok(());
        };

        let file = MediaFile::read_file(path.as_ref())?;
        if !file.has_audio_track() {
            return Err(ErrorKind::Other("Not an audio file!").into())
        }
        let mime = util::sniff_mime_type(&path)?.ok_or(ErrorKind::Other("Cant read mime data"))?;
        let file_extension = path.as_ref().extension().map(|s| {
            s.to_string_lossy().into_owned()
        });

        let metadata = file.get_mediainfo();
        let cover_file = MediaFile::read_file(path.as_ref())?;
        let default_book = NewAudiobook {
            title: metadata.title,
            artist: metadata.metadata.get("artist").cloned(),
            length: metadata.length,
            location: relative_path.to_owned(),
            library_id: self.library.id,
            hash: hash,
            mime_type: mime,
            file_extension: file_extension.unwrap_or("".to_owned())
        };

        let inserted = conn.transaction(|| -> Result<(Audiobook, usize)> {
            let book = Audiobook::ensure_exsits_in(&relative_path, &self.library, &default_book, conn)?;
            book.delete_all_chapters(conn);
            let filename = String::new();
            let chapters = file.get_chapters();
            if let Some(image) = file.get_coverart()? {
                self.save_coverart(&book, &image);
            }
            self.link_audiobook(&book)?;
            let new_chapters: Vec<NewChapter> = chapters.iter().enumerate().map(|(i, chapter)| {
                NewChapter {
                    audiobook_id: book.id,
                    start_time: chapter.start,
                    title: chapter.title.clone(),
                    number: i as i64
                }
            }).collect();
            Ok((book, diesel::insert(&new_chapters).into(chapters::table).execute(&*conn)?))
        });
        match inserted {
            Ok((b, num_chapters)) => {
                info!("Successfully saved book: {} with {} chapters.", b.title, num_chapters);
                Ok(())
            },
            Err(e) => Err(e)
        }
    }

    fn relative_path_str<'a>(&'a self, path: &'a AsRef<Path>) -> Result<&'a str>{
        match path.as_ref().strip_prefix(&self.library.location).map(|p| p.to_str()) {
            Err(_) => Err(ErrorKind::Other("Path is not inside library.").into()),
            Ok(None) => Err(ErrorKind::Other("Path is not a valid utf-8 String.").into()),
            Ok(Some(p)) => Ok(p)
        }
    }

    pub(super) fn create_multifile_audiobook(&self, conn: &diesel::pg::PgConnection, path: &AsRef<Path>) -> Result<()> {
        // This might lead to inconsistent data as we hash before iterating over the files,
        // not better way to go about this seems possible to me
        // TODO: think about this
        let hash = hashing::checksum_dir(path)?;
        let relative_path = self.relative_path_str(path)?.to_owned();
        info!("Scanning multi-file audiobook at {:?}", path.as_ref());

        // if a book with the same hash exists in the database all we want to do is adjust the
        // path to retain all other information related to the book
        //
        // What happens if we have two exact same audiobooks in the library path?:
        // It should just keep switching the paths around whenever a file creation time is
        // updated which is not to bad.
        let done = match Audiobook::update_path(&hash, &relative_path, conn)? {
            Update::Nothing | Update::Path => true,
            Update::NotFound => false
        };
        if done {
            return Ok(());
        };

        let filetype = match probable_audio_filetype(&path)? {
            Some(e) => e,
            None => return Err(ErrorKind::Other("No valid file extensions found.").into())
        };
        debug!("decided on file type {:?}", filetype);
        let walker = WalkDir::new(&path.as_ref())
            .follow_links(true)
            .sort_by(
                |s, o| s.to_string_lossy().humane_cmp(&o.to_string_lossy())
                );
        let mut all_chapters = Vec::new();
        let mut mediafiles = Vec::new();
        let mut start_time = 0.0;
        let title = match path.as_ref().file_name().map(|el| el.to_string_lossy()) {
            Some(s) => s.into_owned(),
            None => return Err(ErrorKind::InvalidUtf8.into())
        };

        let default_book = NewAudiobook {
            length: 0.0,
            library_id: self.library.id,
            location: relative_path.clone(),
            title: title,
            artist: None,
            hash: hash,
            mime_type: filetype.mime_type.clone(),
            file_extension: filetype.extension.to_string_lossy().into_owned()
        };

        let inserted = conn.transaction(||  -> Result<()> {
            let book = Audiobook::ensure_exsits_in(&relative_path, &self.library, &default_book, conn)?;
            book.delete_all_chapters(conn);

            let mut chapter_index = 0;

            for entry in walker {
                match entry {
                    Ok(file) => {
                        if file.path().is_dir() { continue };
                        // TODO: we could check the mimetype and not just the extension here
                        match file.path().extension() {
                            Some(ext) => if ext != filetype.extension { continue },
                            None => { continue }
                        };
                        let media = match MediaFile::read_file(file.path()) {
                            Ok(f) => {
                                let info = f.get_mediainfo();
                                if chapter_index == 0 {
                                    use self::audiobooks::dsl::*;
                                    if let Some(new_title) = info.metadata.get("album") {
                                        diesel::update(audiobooks.filter(id.eq(book.id)))
                                            .set(title.eq(new_title)).execute(conn)?;
                                    }
                                    if let Some(new_artist) = info.metadata.get("artist") {
                                        diesel::update(audiobooks.filter(id.eq(book.id)))
                                            .set(artist.eq(new_artist)).execute(conn)?;
                                    }
                                    let m = MediaFile::read_file(file.path()).unwrap();
                                    if let Some(image) = m.get_coverart()? {
                                        self.save_coverart(&book, &image);
                                    }
                                };
                                let new_chapter = NewChapter {
                                    title: Some(info.title),
                                    start_time: start_time,
                                    audiobook_id: book.id,
                                    number: chapter_index
                                };
                                chapter_index += 1;
                                diesel::insert(&new_chapter).into(chapters::table).execute(conn)?;
                                start_time += info.length;
                                all_chapters.push(new_chapter);
                                f
                            }
                            Err(e) => return Err(e.into())
                        };
                        mediafiles.push(media)
                    },
                    Err(e) => return Err(e.into())
                };
            };
            diesel::update(Audiobook::belonging_to(&self.library).filter(audiobooks::dsl::id.eq(book.id)))
                .set(audiobooks::dsl::length.eq(start_time)).execute(conn)?;
            let target_path = "data/".to_string() +
              &book.id.hyphenated().to_string() + "." +
              &filetype.extension.to_string_lossy().into_owned();
            debug!("muxing files into {:?}", target_path);
            muxer::merge_files(
                &target_path,
                &mediafiles
                )?;
            Ok(())
        });
        match inserted {
            Ok(_) => {
                info!("Successfully saved book");
                Ok(())
            },
            Err(e) => Err(e)
        }
    }
}

fn is_audiobook(path: &Path, regex: &Regex) -> bool {
    regex.is_match(path.to_str().unwrap())
}

///
/// Returns the largest changed time stamp on any file in a given directory
///
fn most_recent_change(path: &AsRef<Path>) -> Result<Option<NaiveDateTime>> {
    // this is a suboptimal solution it doesn't really matter here but creating a vector is not
    // great.
    let times: Result<Vec<NaiveDateTime>> = WalkDir::new(path.as_ref())
        .follow_links(true)
        .into_iter()
        .map(|el| -> Result<NaiveDateTime> {
            match el {
                Ok(f) => {
                    match f.metadata().map(|el| NaiveDateTime::from_timestamp(el.mtime(), el.mtime_nsec() as u32)) {
                        Ok(modified) => Ok(modified),
                        Err(e) => Err(e.into())
                    }
                },
                Err(e) => Err(e.into())
            }
        })
    .collect();
    Ok(times?.iter().max().cloned())
}


/// Find the most common extension in a directory that might be an audio file.
pub(super) fn probable_audio_filetype(path: &AsRef<Path>) -> Result<Option<Filetype>> {
    let mut counts: HashMap<Filetype, usize> = HashMap::new();
    let file_type_iterator = WalkDir::new(path.as_ref())
        .follow_links(true)
        .into_iter()
        .filter_map(|opt| {
            match opt.map(|wd| (wd.path().to_owned(), wd.path().extension().map(|el| el.to_owned()))) {
                Ok((path, Some(o))) => {
                    let mime_type = match util::sniff_mime_type(&path) {
                        Ok(Some(t)) => t,
                        _ => return None
                    };
                    match mime_type.split('/').next() {
                        Some("audio") => (),
                        _ => return None
                    };
                    Some(Filetype {
                        extension: o,
                        mime_type: mime_type
                    })
                },
                _ => None,
            }
        });
    for el in file_type_iterator {
        let mut count = counts.entry(el).or_insert(0);
        *count += 1;
    };
    let mut filetypes: Vec<(Filetype, usize)> = counts.drain().collect();
    filetypes.sort_by(|&(_, v1), &(_, v2)| v2.cmp(&v1));
    Ok(filetypes.pop().map(|el| el.0))
}

/// Determines whether a scan of a path is necessary based on file change data
fn should_scan(path: &Path, last_scan: Option<NaiveDateTime>) -> Result<bool> {
    match most_recent_change(&path)? {
        Some(time) => if let Some(last_scan_time) = last_scan {
            Ok(time >= last_scan_time)
        } else {
            // if there was no scan before we should scan now
            Ok(true)
        },
        None => {
            info!("No change data for files available, will hash everything.");
            Ok(true)
        }
    }
}
