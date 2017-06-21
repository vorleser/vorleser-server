use schema;
use diesel;
use diesel::prelude::*;
use helpers::db::init_db_pool;
use ::*;
use models::library::LibraryAccess;
use models::audiobook::{Audiobook, NewAudiobook};

describe! user_tests {
    before_each {
        let mut pool = init_db_pool();
        let db = pool.get().unwrap();
    }

    after_each {
        db.execute("TRUNCATE audiobooks, chapters, playstates RESTART IDENTITY CASCADE").unwrap();
    }

    it "can access only accessible books and libraries" {
        let user = diesel::insert(&NewUser {
            email: "some@example.com".to_string(),
            password_hash: "hash".to_string()
        }).into(schema::users::table).get_result::<UserModel>(&*db).unwrap();

        let accessible_lib = diesel::insert(&NewLibrary {
            location: "/foo/bar".to_string(),
            is_audiobook_regex: ".*".to_string()
        }).into(schema::libraries::table).get_result::<Library>(&*db).unwrap();
        let inaccessible_lib = diesel::insert(&NewLibrary {
            location: "/foo/baz".to_string(),
            is_audiobook_regex: ".*".to_string()
        }).into(schema::libraries::table).get_result::<Library>(&*db).unwrap();

        diesel::insert(&LibraryAccess {
            library_id: accessible_lib.id,
            user_id: user.id
        }).into(schema::library_permissions::table).get_result::<LibraryAccess>(&*db);

        let books = diesel::insert(&vec![
            NewAudiobook {
                location: "loc1".to_string(),
                mime_type: "mime".to_string(),
                title: "book 1".to_string(),
                length: 1234.5,
                library_id: accessible_lib.id,
                hash: vec![1, 2, 3],
            },
            NewAudiobook {
                location: "loc2".to_string(),
                mime_type: "mime".to_string(),
                title: "book 2".to_string(),
                length: 1232.1,
                library_id: inaccessible_lib.id,
                hash: vec![3, 4, 5],
            },
        ]).into(schema::audiobooks::table).get_results::<Audiobook>(&*db).unwrap();

        assert_eq!(user.accessible_audiobooks(&*db).unwrap(), vec![books[0].clone()]);

        assert_eq!(user.accessible_libraries(&*db).unwrap(), vec![accessible_lib]);
    }
}
