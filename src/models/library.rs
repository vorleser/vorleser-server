use helpers::uuid::Uuid;
use chrono::NaiveDateTime;
use std::time::SystemTime;
use diesel;
use diesel::prelude::*;
use schema::{libraries, audiobooks, library_permissions, self};
use models::audiobook::Audiobook;
use models::library_permission::LibraryPermission;
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

impl Library {
    pub fn create(location: String, audiobook_regex: String, db: &db::Connection) -> Result<Library, diesel::result::Error> {
        db.transaction(|| -> _ {
            debug!("Start transaction creating library.");
            let lib = Library{
                id: Uuid::new_v4(),
                location: location,
                is_audiobook_regex: audiobook_regex,
                last_scan: None
            };
            diesel::insert_into(libraries::table)
                .values(&lib).execute(&*db)?;
            let users: Vec<User> = schema::users::table.load(&*db)?;
            for u in users {
                LibraryPermission::permit(&u, &lib, &*db)?;
            }
            debug!("End transaction creating library.");
            Ok(lib)
        })
    }
}
