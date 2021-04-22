use std::io;
use std::io::Read;
use std::fs::File;
use std::ffi::{OsString, OsStr};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::env;
use std::os::unix::prelude::*;
use std::os::unix::fs;
use std::fs::{create_dir, rename};
use log::error as error_log;

use walkdir::WalkDir;
use walkdir;
use regex::Regex;
use diesel;
use diesel::prelude::*;
use ring::digest;
use humanesort::HumaneOrder;
use chrono::prelude::*;
use chrono::NaiveDateTime;
use crate::helpers::uuid::Uuid;
use diesel::sqlite::SqliteConnection;
use fs2::FileExt;

use crate::config::Config;
use crate::helpers::db::Pool;
use crate::models::library::*;
use crate::models::audiobook::{Audiobook, Update};
use crate::models::chapter::Chapter;
use crate::schema::audiobooks;
use crate::schema::chapters;
use crate::schema::libraries;
use crate::worker::mediafile::MediaFile;
use crate::worker::muxer;
use crate::worker::error::{Result, WorkerError};
use diesel::BelongingToDsl;
use crate::worker::util;
use crate::worker::mediafile::Image;
use super::hashing;

pub struct Scanner {
    pub regex: Regex,
    pub library: Library,
    pub pool: Pool,
    pub config: Config
}

struct MultifileMetadata {
    pub media_files: Vec<MediaFile>,
    pub chapters: Vec<Chapter>,
    pub length: f64,
    pub cover: Option<Image>,
}

#[derive(Eq, PartialEq)]
pub enum LockingBehavior {
    Block,
    Error,
    Dont,
}

#[derive(Hash, Eq, PartialEq, Debug)]
pub(super) struct Filetype {
    pub extension: OsString,
    pub format: String
}

#[derive(Clone)]
enum Scan {
    Incremental,
    Full
}

/// Scanner object for scanning a library.
/// Construct this from a library object, then scan the library's directory using `scan_incremental`.
impl Scanner {
    pub fn new(conn_pool: Pool, library: Library, config: Config) -> Self {
        Self {
            regex: Regex::new(library.is_audiobook_regex.as_str()).expect("Invalid Regex!"),
            library,
            pool: conn_pool,
            config
        }
    }

    fn aquire_lock_file(&mut self, locking_behavior: LockingBehavior) -> Result<()> {
        if locking_behavior == LockingBehavior::Dont { return Ok(()) }
        let mut lock_file_path = PathBuf::from(self.config.data_directory.clone());
        lock_file_path.push("scan.lock");
        let lock_file = File::create(&lock_file_path)?;
        match lock_file.try_lock_exclusive() {
            Err(_) => {
                println!(
                    "It looks like another scan is currently running.\
                    Remove the lockfile {:?} if you are sure no other instance is running.",
                    lock_file_path
                );
                match locking_behavior {
                    LockingBehavior::Block => lock_file.lock_exclusive().map_err(|e| e.into()),
                    _ => Err(WorkerError::Locked.into()),
                }
            }
            Ok(_) => { Ok(()) }
        }
    }

    /// Perform an incremental scan, this takes file change dates into account.
    /// As a result not all files are actually hashed. This should be the default behavior as it
    /// is much faster than hashing all files. If inconsistent situations arise a full scan might
    /// be able to fix the state, depending on what broke.
    pub fn incremental_scan(&mut self, block_on_lock: LockingBehavior) -> Result<()> {
        self.aquire_lock_file(block_on_lock)?;
        self.scan_library(Scan::Incremental)
    }

    /// A full scan actually hashes each file that looks like an audiobook. This should only be run
    /// very sparingly. Maybe on specific user request or on a very long interval. This can easily
    /// keep the filesystem busy for a while if the library is sufficiently large.
    pub fn full_scan(&mut self, block_on_lock: LockingBehavior) -> Result<()> {
        self.aquire_lock_file(block_on_lock)?;
        self.scan_library(Scan::Full)
    }

    /// Gets path for cache directory entry of the book.
    /// This may or may not actually be a file
    fn data_path_of(&self, book: &Audiobook) -> PathBuf {
        PathBuf::from(&format!(
                "{}/{}.{}", self.config.data_directory, book.id.hyphenated(), book.file_extension
                ))
    }

    // for all existing audiobooks
    // check hashes, if changed, remove book and create new with new data
    // if hashes have not changed: check symlinked/remuxed files still there? if not re-link/mux
    fn scan_library(&mut self, scan_type: Scan) -> Result<()> {
        info!("Scanning library: {}", self.library.location);
        let last_scan = self.library.last_scan;
        self.library.last_scan = Some(Utc::now().naive_utc());
        let conn = &*self.pool.get().unwrap();
        self.recover_deleted(conn)?;
        let mut walker = WalkDir::new(&self.library.location).follow_links(true).into_iter();

        self.walk_books(scan_type, walker, last_scan, conn);

        self.delete_not_in_fs(conn)?;
        
        match diesel::update(libraries::dsl::libraries.filter(libraries::dsl::id.eq(&self.library.id)))
            .set(&self.library)
            .execute(conn) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.into())
            }
    }

    fn walk_books(&self, scan_type: Scan, mut walker: walkdir::IntoIter,
                  last_scan: Option<chrono::NaiveDateTime>, conn: &SqliteConnection) -> Result<()> {
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
                let r = self.handle_book_at_path(conn, scan_type.clone(), path, relative_path, last_scan);

                match r {
                    Ok(_) => {},
                    Err(e) => error_log!("Error while processing {}: {}", path.display(), e),
                }

                // Since we are in an audiobook we don't continue searching deeper in the dir tree from here
                if path.is_dir() {
                    walker.skip_current_dir();
                }
            };
            ()
        }
        Ok(())
    }

    fn handle_book_at_path(&self, conn: &SqliteConnection, scan_type: Scan, path: &Path, relative_path: &Path,
                           last_scan: Option<chrono::NaiveDateTime>) -> Result<()> {
        use crate::schema::audiobooks::dsl::location;

        match scan_type {
            Scan::Incremental => {
                let preexisting_book = Audiobook::belonging_to(&self.library)
                    .filter(location.eq(&relative_path.to_string_lossy()))
                    .first::<Audiobook>(conn).optional()?;
                if should_scan(path, last_scan)? || preexisting_book.is_none() {
                    self.process_audiobook(&path, conn)?;
                }
            },
            Scan::Full => self.process_audiobook(&path, conn)?
        }

        let mut book_result = Audiobook::belonging_to(&self.library)
            .filter(location.eq(&relative_path.to_string_lossy()))
            .get_result::<Audiobook>(&*conn);
        debug!("book: {:?}", book_result);

        // Ensure cached file exists here no need to check if its current, that is ensured
        // above
        if let Ok(mut book) = book_result {
            if path.is_dir() && !self.data_path_of(&book).exists() {
                debug!("No remuxed version of {}, remuxing!", book.title);
                match self.multifile_remux(&mut book) {
                    Ok(_) => info!("Successfully remuxed {}", book.title),
                    Err(e) => info!("Error {:?} while remuxing {}", e, book.title),
                }
            } else if !self.data_path_of(&book).exists() {
                debug!("No remuxed version of {}, linking!", book.title);
                match self.link_audiobook(&book) {
                    Ok(_) => info!("Successfully linked {} into collection", book.title),
                    Err(e) => info!("Error {:?} while linking {}", e, book.title),
                }
            }
        }
        Ok(())
    }

    fn process_audiobook(&self, path: &dyn AsRef<Path>, conn: &SqliteConnection) -> Result<()> {
        if path.as_ref().is_dir() {
            self.create_multifile_audiobook(conn, path)
        } else {
            self.create_audiobook(conn, path)
        }
    }


    /// Try to recover those books that were marked as deleted.
    /// Checks the file paths of books in the database and recovers them if hashes match
    fn recover_deleted(&self, conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::audiobooks::dsl as dsl;
        let mut recovered = 0;
        for book in Audiobook::belonging_to(&self.library).filter(dsl::deleted.eq(true)).get_results::<Audiobook>(&*conn)? {
            let path = Path::new(&self.library.location).join(Path::new(&book.location));
            if !path.exists() { continue }

            info!("Recovering previously deleted book: {:?}", path);
            let hash = if path.is_dir() {
                hashing::checksum_dir(&path)?
            } else {
                hashing::checksum_file(&path)?
            };

            if hash == book.hash {
                use crate::schema::audiobooks::dsl::*;
                diesel::update(
                        Audiobook::belonging_to(&self.library)
                        .filter(dsl::id.eq(book.id))
                    )
                    .set(dsl::deleted.eq(false))
                    .execute(&*conn)?;
                recovered += 1;
            }
        }
        Ok(recovered)
    }

    /// Delete all those books from the database that are not present in the file system.
    fn delete_not_in_fs(&self, conn: &SqliteConnection) -> Result<()> {
        debug!("looking for removed books");

        for book in Audiobook::belonging_to(&self.library).get_results::<Audiobook>(&*conn)? {
            let path = Path::new(&self.library.location).join(Path::new(&book.location));

            debug!("checking {:?}", path);
            if !path.exists() && !book.deleted {
                info!("The book at {:?} seems to have gone away, marking as deleted", path);

                use crate::schema::audiobooks::dsl::*;
                let del = diesel::update(
                        Audiobook::belonging_to(&self.library)
                        .filter(id.eq(book.id))
                    )
                    .set(deleted.eq(true))
                    .execute(&*conn)?;
                debug!("deleted: {}", del);
                match del {
                    0 => warn!("Could not delete audiobook, is something wrong with the DB?"),
                    1 => {},
                    x => warn!("Deleted multiple audiobooks with same UUID, database integrity might be compromised."),
                }
            }
        };
        Ok(())
    }


    /// Save cover art to directory
    fn save_coverart(&self, book: &Audiobook, image: &Image) -> Result<()> {
        let mut dest = PathBuf::from(format!("{}/img", self.config.data_directory));
        if let Err(e) = create_dir(dest.clone()) {
            match e.kind() {
                io::ErrorKind::AlreadyExists => (),
                _ => Err(e)?
            };
        };
        dest.push(&book.id.hyphenated().to_string());
        image.save(&dest)?;
        Ok(())
    }

    pub(super) fn create_audiobook(&self, conn: &diesel::sqlite::SqliteConnection, path: &dyn AsRef<Path>) -> Result<()> {
        info!("Scanning single file audiobook at: {:?}", path.as_ref());
        let relative_path = self.relative_path_str(path)?;
        let hash = hashing::checksum_file(path)?;

        let done = match Audiobook::update_path(&hash, &relative_path, conn)? {
            Update::Nothing | Update::Path => true,
            Update::NotFound => false
        };
        if done {
            debug!("This audiobook already exists in the database, moving on.");
            return Ok(());
        };

        let file = MediaFile::read_file(path.as_ref())?;
        if !file.has_audio_track() {
            return Err(WorkerError::NotAnAudioFile.into())
        }
        let file_extension = path.as_ref().extension().map(|s| {
            s.to_string_lossy().into_owned()
        });

        let metadata = file.get_mediainfo();
        let cover_file = MediaFile::read_file(path.as_ref())?;
        let default_book = Audiobook {
            id: Uuid::new_v4(),
            title: metadata.title,
            artist: metadata.metadata.get("artist").cloned(),
            length: metadata.length,
            location: relative_path.to_owned(),
            library_id: self.library.id,
            hash,
            file_extension: file_extension.unwrap_or_else(|| "".to_owned()),
            deleted: false,
        };

        let chapters = file.get_chapters();
        let maybe_image = file.get_coverart()?;

        let inserted = conn.exclusive_transaction(|| -> Result<(Audiobook, usize)> {
            debug!("Start transaction inserting single audiobook.");
            let book = Audiobook::ensure_exists_in(
                &relative_path, &self.library, &default_book, conn
            )?;
            book.delete_all_chapters(conn);
            if let Some(image) = maybe_image {
                self.save_coverart(&book, &image);
            };
            self.link_audiobook(&book)?;
            let new_chapters: Vec<Chapter> = chapters.iter().enumerate().map(|(i, chapter)| {
                Chapter {
                    id: Uuid::new_v4(),
                    audiobook_id: book.id,
                    start_time: chapter.start,
                    title: chapter.title.clone(),
                    number: i as i64
                }
            }).collect();
            debug!("End transaction inserting single audiobook.");
            Ok((book, diesel::replace_into(chapters::table)
                .values(&new_chapters).execute(&*conn)?))
        });
        match inserted {
            Ok((b, num_chapters)) => {
                info!("Successfully saved book: {} with {} chapters.", b.title, num_chapters);
                Ok(())
            },
            Err(e) => Err(e)
        }
    }

    /// Audiobooks that are not remuxed are linked into our data directory so we have one canonical
    /// source of data.
    fn link_audiobook(&self, book: &Audiobook) -> Result<()> {
        let mut dest = PathBuf::from(self.config.data_directory.to_owned());
        dest.push(&book.id.hyphenated().to_string());
        dest.set_extension(&book.file_extension);
        let mut src = PathBuf::from(&self.library.location);
        src.push(&book.location);
        fs::symlink(src, dest);
        Ok(())
    }

    fn multifile_remux(&self, mut book: &mut Audiobook) -> Result<()> {
        let collection = self.multifile_extract_chapters(&mut book)?;
        let target_path = self.data_path_of(&book);
        muxer::merge_files(
            &target_path,
            &collection.media_files
            )?;
        Ok(())
    }


    fn multifile_extract_chapters(&self, book: &mut Audiobook) -> Result<MultifileMetadata> {
        let book_path = Path::new(&self.library.location).join(book.location.clone());
        let walker = WalkDir::new(book_path)
            .follow_links(true)
            .sort_by(
                |s, o| s.path().to_string_lossy().humane_cmp(&o.path().to_string_lossy())
            );

        let mut all_chapters: Vec<Chapter> = Vec::new();
        let mut mediafiles = Vec::new();
        let mut start_time = 0.0;
        let mut chapter_index = 0;
        let mut cover: Option<Image> = None;

        for entry in walker {
            match entry {
                Ok(file) => {
                    if file.path().is_dir() { continue };
                    match file.path().extension() {
                        Some(ext) => if (ext.to_string_lossy()) != book.file_extension { continue },
                        None => { continue }
                    };
                    let media = match MediaFile::read_file(file.path()) {
                        Ok(f) => {
                            let info = f.get_mediainfo();
                            if chapter_index == 0 {
                                use self::audiobooks::dsl::*;
                                if let Some(new_title) = info.metadata.get("album") {
                                    book.title = new_title.to_owned();
                                }
                                if let Some(new_artist) = info.metadata.get("artist") {
                                    book.artist = Some(new_artist.to_owned());
                                }
                                let m = MediaFile::read_file(file.path()).unwrap();
                                cover = m.get_coverart()?;
                            };
                            if Some(&info.title) != all_chapters.last().and_then(|c| c.title.as_ref() ) {
                                let new_chapter = Chapter {
                                    id: Uuid::new_v4(),
                                    title: Some(info.title),
                                    start_time,
                                    audiobook_id: book.id,
                                    number: chapter_index
                                };
                                chapter_index += 1;
                                all_chapters.push(new_chapter);
                            }
                            start_time += info.length;
                            f
                        }
                        Err(e) => return Err(e)
                    };
                    mediafiles.push(media)
                },
                Err(e) => return Err(e.into())
            };
        };

        Ok(MultifileMetadata {
            media_files: mediafiles,
            chapters: all_chapters,
            length: start_time,
            cover: cover,
        })
    }

    pub(super) fn create_multifile_audiobook(&self, conn: &diesel::sqlite::SqliteConnection, path: &dyn AsRef<Path>) -> Result<()> {
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
        debug!("Checking if {} is up to date, result is: {}", relative_path, done);
        if done {
            debug!("This audiobook already exists in the database, moving on.");
            return Ok(());
        };

        let filetype = match probable_audio_filetype(&path)? {
            Some(e) => e,
            None => return Err(WorkerError::NoValidFileExtensions.into())
        };

        debug!("decided on file type {:?}", filetype);

        let title = match path.as_ref().file_name().map(|el| el.to_string_lossy()) {
            Some(s) => s.into_owned(),
            None => return Err(WorkerError::InvalidUtf8.into())
        };

        let mut default_book = Audiobook {
            id: Uuid::new_v4(),
            length: 0.0,
            library_id: self.library.id,
            location: relative_path.clone(),
            title,
            artist: None,
            hash,
            file_extension: filetype.to_owned().into_string().unwrap(),
            deleted: false
        };

        let temp_target_path = self.build_target_path(
            &self.config.data_directory, &default_book.id, &filetype
        );

        let collection = self.multifile_extract_chapters(&mut default_book)?;
        debug!("muxing files into {:?}", temp_target_path);
        muxer::merge_files(
            &temp_target_path,
            &collection.media_files
        )?;


        let inserted = conn.exclusive_transaction(||  -> Result<Audiobook> {
            debug!("Start transaction inserting multifile audiobook.");
            let mut book = Audiobook::ensure_exists_in(
                &relative_path, &self.library, &default_book, conn
            )?;

            if let Some(img) = collection.cover {
                self.save_coverart(&book, &img);
            }

            book.length = collection.length;
            book.delete_all_chapters(conn);
            for new_chapter in collection.chapters {
                diesel::insert_into(chapters::table).values(&new_chapter).execute(conn)?;
            }

            diesel::update(
                Audiobook::belonging_to(&self.library).filter(audiobooks::dsl::id.eq(&book.id))
            ).set(&book).execute(conn)?;

            let target_path = self.build_target_path(
                &self.config.data_directory, &book.id, &filetype
            );
            debug!("Moving {} to {}.", temp_target_path, target_path);
            rename(temp_target_path, target_path)?;
            debug!("End transaction inserting multifile audiobook.");
            Ok(book)
        });
        match inserted {
            Ok(book) => {
                info!("Successfully saved book: {}", book.title);
                Ok(())
            },
            Err(e) => {
                warn!("Error saving book: {}", relative_path);
                Err(e)
            }
        }
    }

    fn relative_path_str<'a>(&'a self, path: &'a dyn AsRef<Path>) -> Result<&'a str>{
        match path.as_ref().strip_prefix(&self.library.location).map(|p| p.to_str()) {
            Err(_) => Err(WorkerError::OutsideLibrary.into()),
            Ok(None) => Err(WorkerError::InvalidUtf8.into()),
            Ok(Some(p)) => Ok(p)
        }
    }

    /// Path at which to place a data file, where uuid is the books uuid and filetype its filetype.
    fn build_target_path(&self, data_path: &dyn AsRef<str>, uuid: &Uuid, filetype: &OsStr) -> String {
        format!(
            "{}/{}.{}",
            data_path.as_ref(),
            uuid.hyphenated(),
            &filetype.to_string_lossy()
        )
    }
}

fn is_audiobook(path: &Path, regex: &Regex) -> bool {
    regex.is_match(path.to_str().unwrap())
}

///
/// Returns the largest changed time stamp on any file in a given directory
///
fn most_recent_change(path: &dyn AsRef<Path>) -> Result<Option<NaiveDateTime>> {
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
pub(super) fn probable_audio_filetype(path: &dyn AsRef<Path>) -> Result<Option<OsString>> {
    let mut counts: HashMap<OsString, usize> = HashMap::new();
    let file_type_iterator = WalkDir::new(path.as_ref())
        .follow_links(true)
        .into_iter()
        .filter_map(|opt| {
            match opt.map(|wd| wd.path().extension().map(|el| el.to_owned())) {
                Ok(Some(ext)) => Some(ext),
                _ => None
            }
        });
    for el in file_type_iterator {
        let mut count = counts.entry(el).or_insert(0);
        *count += 1;
    };
    let mut filetypes: Vec<(OsString, usize)> = counts.drain().collect();
    filetypes.sort_by(|&(_, count_left), &(_, count_right)| count_left.cmp(&count_right));
    Ok(filetypes.pop().map(|el| el.0))
}

/// Determines whether a scan of a path is necessary based on file change data
fn should_scan(path: &Path, last_scan: Option<NaiveDateTime>) -> Result<bool> {
    match most_recent_change(&path)? {
        Some(recent_change_time) => if let Some(last_scan_time) = last_scan {
            debug!("Should scan based on time stamps is: {:?} >= {:?} meaning: {}",
                  recent_change_time,
                  last_scan_time,
                  recent_change_time >= last_scan_time);
            Ok(recent_change_time >= last_scan_time)
        } else {
            debug!("First scan of this book ever, scanning!");
            // if there was no scan before we should scan now
            Ok(true)
        },
        None => {
            debug!("No change data for files available, will hash everything.");
            Ok(true)
        }
    }
}
