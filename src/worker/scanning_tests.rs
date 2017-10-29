use chrono::NaiveDate;
use std::path::{Path, PathBuf};
use std::process::Command;
use diesel::prelude::*;
use diesel;
use walkdir::WalkDir;

use ::worker::util;
use helpers::db::init_test_db_pool;

fn set_time(file: &str, date: &NaiveDate) {
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


fn set_times(times: Vec<(String, NaiveDate)>) {
    for (ref path, ref date) in times {
        set_time(path, date);
    }
}

// IMPORTANT:
// Each test has exactly one directory in `integration-tests`.
// We don't have locking on this so it is extremely important that no two tests share the same
// directory.
//
// To ensure this please name each test EXACTLY like the directory.
describe! scanner_integrationn_tests {
    before_each {
        let mut pool = init_test_db_pool();
        util::shut_up_ffmpeg();

        use models::audiobook::{Audiobook, NewAudiobook, Update};
        use models::library::{NewLibrary, Library};
        use schema::libraries;
        use worker::scanner;
        let new_lib = NewLibrary{
            location: "".to_owned(),
            is_audiobook_regex: "^[^/]+$".to_owned()
        };
        let library: Library = diesel::insert(&new_lib)
            .into(libraries::table)
            .get_result(&*(pool.get().unwrap()))
            .unwrap();
        let mut scanner = scanner::Scanner::new(pool.clone(), library);
    }

    it "simple" {
        // Time step 01:
        let base = String::from("integration-tests/simple/01");
        scanner.library.location = base.clone();
        set_time(&(base + "/book.mp3"), &NaiveDate::from_ymd(1990, 1, 1));
        scanner.incremental_scan();
        assert_eq!(1, Audiobook::belonging_to(&scanner.library).count().first::<i64>(&*(pool.get().unwrap())).unwrap());
    }

    it "simple_deletion" {
        // Time step 01:
        println!("============Step 1!============");
        let mut base = String::from("integration-tests/simple_deletion/01");
        scanner.library.location = base.clone();
        set_time(&base, &NaiveDate::from_ymd(1990, 1, 1));
        scanner.incremental_scan().unwrap();
        assert_eq!(1, Audiobook::belonging_to(&scanner.library).count().first::<i64>(&*(pool.get().unwrap())).unwrap());
        println!("============Step 2!============");
        // Time step 02:
        base = String::from("integration-tests/simple_deletion/02");
        scanner.library.location = base.clone();
        scanner.incremental_scan().unwrap();
        use schema::audiobooks::dsl::deleted;
        assert_eq!(0, Audiobook::belonging_to(&scanner.library).filter(deleted.eq(false)).count().first::<i64>(&*(pool.get().unwrap())).unwrap());
    }

    it "ignore_other_files" {
        // Time step 01:
        let base = String::from("integration-tests/ignore_other_files/01");
        scanner.library.location = base.clone();
        scanner.incremental_scan();
        assert_eq!(0, Audiobook::belonging_to(&scanner.library).count().first::<i64>(&*(pool.get().unwrap())).unwrap());
    }

    // it "recovers_deleted_same_timestamp" {
    //     // Time step 01:
    //     let mut base = String::from("integration-tests/recovers_deleted_same_timestamp/01");
    //     scanner.library.location = base.clone();
    //     set_time(&base, &NaiveDate::from_ymd(1990, 1, 1));
    //     scanner.incremental_scan();
    //     assert_eq!(1, Audiobook::belonging_to(&scanner.library).count().first::<i64>(&*(pool.get().unwrap())).unwrap());

    //     // Time step 02:
    //     base = String::from("integration-tests/recovers_deleted_same_timestamp/02");
    //     scanner.library.location = base.clone();
    //     scanner.incremental_scan();
    //     assert_eq!(0, Audiobook::belonging_to(&scanner.library).count().first::<i64>(&*(pool.get().unwrap())).unwrap());

    //     // Time step 03:
    //     base = String::from("integration-tests/recovers_deleted_same_timestamp/03");
    //     set_time(&base, &NaiveDate::from_ymd(1990, 1, 1));
    //     scanner.library.location = base.clone();
    //     scanner.incremental_scan();
    //     assert_eq!(1, Audiobook::belonging_to(&scanner.library).count().first::<i64>(&*(pool.get().unwrap())).unwrap());

    //     // TODO: Make sure the id is the same
    // }
}
