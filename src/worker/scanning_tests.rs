use chrono::NaiveDate;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs::File;
use diesel::prelude::*;
use diesel;
use walkdir::WalkDir;

use ::worker::util;
use helpers::db::init_test_db_pool;
use helpers::db::Pool;
use models::library::Library;
use models::audiobook::Audiobook;
use worker::scanner::{Scanner, LockingBehavior};
use helpers::uuid::Uuid;
use config;

fn set_date(file: &str, date: &NaiveDate) {
    let time = date.format("%y%m%d0000").to_string();
    for entry in WalkDir::new(file) {
        let e = entry.unwrap();
        let path = e.path().clone();
        Command::new("touch")
            .arg("-t")
            .arg(time.clone())
            .arg(path)
            .output()
            .expect("Can't run touch!");
    }
}

fn data_cover_file(book: &Audiobook) -> PathBuf {
    PathBuf::from(format!("data/img/{}", &book.id.hyphenated()))
}

fn data_file(book: &Audiobook) -> PathBuf {
    PathBuf::from(
        format!("data/{}.{}", &book.id.hyphenated(), &book.file_extension)
    )
}

fn count_books(scanner: &Scanner, pool: &Pool) -> i64 {
    use models::audiobook::Audiobook;
    use schema::audiobooks::dsl::deleted;
    Audiobook::belonging_to(&scanner.library)
               .filter(deleted.eq(false))
               .count()
               .first::<i64>(&*(pool.get().unwrap())).unwrap()
}


fn set_dates(times: Vec<(String, NaiveDate)>) {
    for (ref path, ref date) in times {
        set_date(path, date);
    }
}

fn all_books(scanner: &Scanner, pool: &Pool) -> Vec<Audiobook> {
    use schema::audiobooks::dsl::deleted;
    Audiobook::belonging_to(&scanner.library)
        .filter(deleted.eq(false))
        .load(&pool.get().unwrap()).unwrap()
}

macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            extern crate core;
            unsafe { core::intrinsics::type_name::<T>() }
        }
        let name = type_name_of(f);
        &name[6..name.len() - 4]
    }}
}

macro_rules! data_path{
    ($name: expr) => {
        format!("integration-tests/{}/{}", function!().split("::").last().unwrap(), $name)
    }
}

// IMPORTANT:
// Each test has exactly one directory in `integration-tests`.
// We don't have locking on this so it is extremely important that no two tests share the same
// directory.
//
// To ensure this please name each test EXACTLY like the directory.
speculate! {
    before {
        let mut pool = init_test_db_pool();
        util::shut_up_ffmpeg();

        use models::audiobook::{Audiobook, Update};
        use models::library::Library;
        use schema::libraries;
        use worker::scanner;
        let library = Library{
            id: Uuid::new_v4(),
            location: "".to_owned(),
            is_audiobook_regex: "^[^/]+$".to_owned(),
            last_scan: None,
        };
        diesel::insert_into(libraries::table)
            .values(&library)
            .execute(&*(pool.get().unwrap()))
            .unwrap();
        let mut scanner = scanner::Scanner::new(
            pool.clone(),
            library,
            config::load_config_from_path(&"test-data/test-config.toml").unwrap()
        );
    }

    describe "scanner_integration_tests" {
        it "discovers books" {
            // Time step 01:
            let base = String::from("integration-tests/simple/01");
            scanner.library.location = base.clone();
            set_date(&(base + "/book.mp3"), &NaiveDate::from_ymd(1990, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont);
            assert_eq!(1, count_books(&scanner, &pool));
        }

        it "can delete books" {
            // Time step 01:
            println!("============Step 1!============");
            let mut base = String::from("integration-tests/simple_deletion/01");
            scanner.library.location = base.clone();
            set_date(&base, &NaiveDate::from_ymd(1990, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont).unwrap();
            assert_eq!(1, Audiobook::belonging_to(&scanner.library).count().first::<i64>(&*(pool.get().unwrap())).unwrap());

            println!("============Step 2!============");
            // Time step 02:
            base = String::from("integration-tests/simple_deletion/02");
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont).unwrap();
            use schema::audiobooks::dsl::deleted;
            assert_eq!(0, count_books(&scanner, &pool));
        }

        it "ignores other files" {
            // Time step 01:
            let base = String::from("integration-tests/ignore_other_files/01");
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            assert_eq!(0, count_books(&scanner, &pool));
        }

        it "recovers deleted books with the same timestamp" {
            use schema::audiobooks::dsl::deleted;
            // Time step 01:
            let mut base = String::from("integration-tests/recovers_deleted_same_timestamp/01");
            scanner.library.location = base.clone();
            set_date(&base, &NaiveDate::from_ymd(1990, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont);
            let book_1 = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert_eq!(1, count_books(&scanner, &pool));

            // Time step 02:
            base = String::from("integration-tests/recovers_deleted_same_timestamp/02");
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            assert_eq!(0, count_books(&scanner, &pool));

            // Time step 03:
            base = String::from("integration-tests/recovers_deleted_same_timestamp/03");
            set_date(&base, &NaiveDate::from_ymd(1990, 1, 1));
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            let book_2 = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert_eq!(1, count_books(&scanner, &pool));
            assert_eq!(book_1.id, book_2.id);
        }

        it "works_with_moved_files" {
            use schema::audiobooks::dsl::deleted;
            println!("============Step 1!============");
            let mut base = String::from("integration-tests/works_with_moved_files/01");
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            let book = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert_eq!(1, count_books(&scanner, &pool));

            println!("============Step 2!============");
            let mut base = String::from("integration-tests/works_with_moved_files/02");
            scanner.library.location = base.clone();
            let book2 = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            set_date(&base, &NaiveDate::from_ymd(2050, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont);
            assert_eq!(1, count_books(&scanner, &pool));
            assert_eq!(book.id, book2.id);
        }

        it "works_with_moved_files_same_name" {
            // This tests introduces another file of the same name in the second step
            // The file from the first step is still moved
            // We don't feel strongly about how this behaves we would just like to know when it changes
            // Currently the filename takes precedence over the files hash.
            // We want this behavior because
            use schema::audiobooks::dsl::deleted;
            println!("============Step 1!============");
            let mut base = String::from("integration-tests/works_with_moved_files_same_name/01");
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            let book = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert_eq!(book.location, "book.mp3");
            assert_eq!(1, count_books(&scanner, &pool));

            println!("============Step 2!============");
            let mut base = String::from("integration-tests/works_with_moved_files_same_name/02");
            scanner.library.location = base.clone();
            set_date(&base, &NaiveDate::from_ymd(2050, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont);
            let book2 = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert_eq!(2, count_books(&scanner, &pool));
            assert_eq!(book.id, book2.id);
            // todo: this doesn't behave like we want it to. fix and update test
            assert_ne!(book2.location, "book.mp3");
        }

        it "works_with_moved_multifile" {
            let s1 = data_path!("01");

            scanner.library.location = s1.clone();
            set_date(&s1, &NaiveDate::from_ymd(1990, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont);

            assert_eq!(count_books(&scanner, &pool), 1);
            let book = all_books(&scanner, &pool).first().unwrap().clone();


            let s2 = data_path!("02");

            scanner.library.location = s2.clone();
            set_date(&s2, &NaiveDate::from_ymd(1990, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont);
            let book_moved = all_books(&scanner, &pool).first().unwrap().clone();

            assert_eq!(count_books(&scanner, &pool), 1);
            assert_eq!(book_moved.location, "book_moved");
            assert_eq!(book_moved.id, book.id);
        }

        it "content_changed" {
            use schema::audiobooks::dsl::deleted;
            println!("============Step 1!============");
            let mut base = String::from("integration-tests/content_changed/01");
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            let book = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert_eq!(book.location, "book.mp3");
            assert_eq!(1, count_books(&scanner, &pool));

            println!("============Step 2!============");
            let mut base = String::from("integration-tests/content_changed/02");
            scanner.library.location = base.clone();
            let book2 = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert!(!data_file(&book2).exists());
            set_date(&base, &NaiveDate::from_ymd(2050, 1, 1));
            scanner.incremental_scan(LockingBehavior::Dont);
            assert_eq!(1, count_books(&scanner, &pool));
            assert_eq!(book.id, book2.id);
        }

        it "content_changed_multifile" {
            use schema::audiobooks::dsl::deleted;
            println!("============Step 1!============");
            let mut base = String::from("integration-tests/content_changed_multifile/01");
            set_date(&base, &NaiveDate::from_ymd(2008, 1, 1));
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            let book = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            let file_1 = File::open(&data_file(&book)).unwrap();
            let changed_1 = file_1.metadata().unwrap().modified().unwrap();
            assert!(data_file(&book).exists());
            assert_eq!(book.location, "book");
            assert_eq!(1, count_books(&scanner, &pool));

            println!("============Step 2!============");
            let mut base = String::from("integration-tests/content_changed_multifile/02");
            set_date(&base, &NaiveDate::from_ymd(2050, 1, 1));
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);

            let book2 = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            println!("{:?}", book2);

            // Make sure the file was remuxed again
            let file_2 = File::open(&data_file(&book2)).unwrap();
            let changed_2 = file_2.metadata().unwrap().modified().unwrap();

            assert_ne!(book.length, book2.length);
            println!("{:?} > {:?}", changed_2, changed_1);
            assert!(changed_2 > changed_1);

            assert_eq!(1, count_books(&scanner, &pool));
            assert_eq!(book.id, book2.id);
        }

        it "cover_changed_multifile" {
            use schema::audiobooks::dsl::deleted;
            println!("============Step 1!============");
            let mut base = data_path!("01");
            println!("{}", base);
            set_date(&base, &NaiveDate::from_ymd(2008, 1, 1));
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);
            let book = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            assert!(!data_cover_file(&book).exists());

            println!("============Step 2!============");
            let mut base = data_path!("02");
            set_date(&base, &NaiveDate::from_ymd(2050, 1, 1));
            scanner.library.location = base.clone();
            scanner.incremental_scan(LockingBehavior::Dont);

            let book2 = Audiobook::belonging_to(&scanner.library)
                .filter(deleted.eq(false))
                .first::<Audiobook>(&*(pool.get().unwrap())).unwrap();
            println!("{:?}", book2);

            assert!(data_cover_file(&book).exists());
        }
    }
}
