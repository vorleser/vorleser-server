use schema;
use diesel;
use diesel::prelude::*;
use helpers::db::init_test_db_pool;
use ::*;
use models::user::{NewUser, User};
use models::library::{LibraryAccess, Library};
use models::audiobook::Audiobook;
use helpers::uuid::Uuid;

describe! user_tests {
    before_each {
        let mut pool = init_test_db_pool();
        let db = pool.get().unwrap();
    }

    after_each {
        db.execute("TRUNCATE audiobooks, chapters, playstates RESTART IDENTITY CASCADE").unwrap();
    }

    it "can access only accessible books and libraries" {
        let user = User::create(&"some@example.com", &"password", &*db).unwrap();

        let accessible_lib = Library {
            id: Uuid::new_v4(),
            location: "/foo/bar".to_string(),
            is_audiobook_regex: ".*".to_string(),
            last_scan: None,
        };
        diesel::insert_into(schema::libraries::table)
            .values(&accessible_lib).execute(&*db).unwrap();

        let inaccessible_lib = Library {
            id: Uuid::new_v4(),
            location: "/foo/baz".to_string(),
            is_audiobook_regex: ".*".to_string(),
            last_scan: None,
        };
        diesel::insert_into(schema::libraries::table)
            .values(&inaccessible_lib).execute(&*db).unwrap();

        diesel::insert_into(schema::library_permissions::table).values(&LibraryAccess {
            library_id: accessible_lib.id.clone(),
            user_id: user.id.clone()
        }).execute(&*db);

        let books = vec![
            Audiobook {
                id: Uuid::new_v4(),
                location: "loc1".to_string(),
                title: "book 1".to_string(),
                artist: Some("artist 1".to_string()),
                length: 1234.5,
                library_id: accessible_lib.id.clone(),
                hash: vec![1, 2, 3],
                file_extension: ".mp3".to_owned(),
                deleted: false,
            },
            Audiobook {
                id: Uuid::new_v4(),
                location: "loc2".to_string(),
                title: "book 2".to_string(),
                artist: None,
                length: 1232.1,
                library_id: inaccessible_lib.id,
                hash: vec![3, 4, 5],
                file_extension: ".mp3".to_owned(),
                deleted: false,
            },
        ];

        diesel::insert_into(schema::audiobooks::table).values(&books).execute(&*db).unwrap();

        assert_eq!(user.accessible_audiobooks(&*db).unwrap(), vec![books[0].clone()]);

        assert_eq!(user.accessible_libraries(&*db).unwrap(), vec![accessible_lib]);
    }
}
