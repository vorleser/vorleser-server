use uuid::Uuid;
use chrono::NaiveDateTime;
use std::time::SystemTime;
use diesel;
use diesel::prelude::*;
use schema::{libraries, audiobooks, library_permissions};
use schema;
use models::audiobook::Audiobook;
use helpers::db;
use models::user::User;

#[table_name="libraries"]
#[derive(PartialEq, Debug, Clone, AsChangeset, Queryable, Associations, Identifiable, Serialize,
         Insertable)]
#[has_many(audiobooks, library_permissions)]
pub struct Library {
    pub id: Uuid,
    #[serde(skip_serializing)]
    pub location: String,
    #[serde(skip_serializing)]
    pub is_audiobook_regex: String,
    #[serde(skip_serializing)]
    pub last_scan: Option<NaiveDateTime>
}

type LibraryColumns = (
    libraries::id,
    libraries::location,
    libraries::is_audiobook_regex,
    libraries::last_scan,
);

pub const LIBRARY_COLUMNS: LibraryColumns = (
    libraries::id,
    libraries::location,
    libraries::is_audiobook_regex,
    libraries::last_scan,
);

#[table_name="library_permissions"]
#[primary_key(library_id, user_id)]
#[derive(Debug, Clone, Queryable, Associations, Identifiable, Insertable)]
pub struct LibraryAccess {
    pub library_id: Uuid,
    pub user_id: Uuid,
}

impl LibraryAccess {
    pub fn permit(user: &User, library: &Library, db: &db::Connection) -> Result<LibraryAccess, diesel::result::Error> {
        diesel::insert_into(library_permissions::table)
            .values(&LibraryAccess {
                library_id: library.id,
                user_id: user.id
            }).get_result(&*db)
    }
}

impl Library {
    pub fn create(location: String, audiobook_regex: String, db: &db::Connection) -> Result<Library, diesel::result::Error> {
        db.transaction(|| -> _ {
            let lib = diesel::insert_into(libraries::table)
                .values(&Library{
                    id: Uuid::new_v4(),
                    location: location,
                    is_audiobook_regex: audiobook_regex,
                    last_scan: None
                }).get_result::<Library>(&*db)?;
            let users: Vec<User> = schema::users::table.load(&*db)?;
            for u in users {
                LibraryAccess::permit(&u, &lib, &*db)?;
            }
            Ok(lib)
        })
    }
}
