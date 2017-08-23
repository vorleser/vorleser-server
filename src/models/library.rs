use uuid::Uuid;
use chrono::NaiveDateTime;
use std::time::SystemTime;
use diesel;
use diesel::prelude::*;
use schema::{libraries, audiobooks, library_permissions};
use schema;
use models::audiobook::Audiobook;
use helpers::db;
use models::user::UserModel;

#[table_name="libraries"]
#[derive(Insertable)]
pub struct NewLibrary {
    pub location: String,
    pub is_audiobook_regex: String
}

#[table_name="libraries"]
#[derive(PartialEq, Debug, Clone, AsChangeset, Queryable, Associations, Identifiable, Serialize)]
#[has_many(audiobooks, library_permissions)]
pub struct Library {
    pub id: Uuid,
    pub content_change_date: NaiveDateTime,
    #[serde(skip_serializing)]
    pub location: String,
    #[serde(skip_serializing)]
    pub is_audiobook_regex: String,
    #[serde(skip_serializing)]
    pub last_scan: Option<NaiveDateTime>
}

#[table_name="library_permissions"]
#[primary_key(library_id, user_id)]
#[derive(Debug, Clone, Queryable, Associations, Identifiable, Insertable)]
pub struct LibraryAccess {
    pub library_id: Uuid,
    pub user_id: Uuid,
}

impl LibraryAccess {
    pub fn permit(user: &UserModel, library: &Library, db: &db::Connection) -> Result<LibraryAccess, diesel::result::Error> {
        diesel::insert(&LibraryAccess {
            library_id: library.id,
            user_id: user.id
        }).into(library_permissions::table).get_result(&*db)
    }
}

impl Library {
    pub fn create(location: String, audiobook_regex: String, db: &db::Connection) -> Result<Library, diesel::result::Error> {
        db.transaction(|| -> _ {
            let lib = diesel::insert(&NewLibrary{
                location: location,
                is_audiobook_regex: audiobook_regex
            }).into(libraries::table).get_result::<Library>(&*db)?;
            let users: Vec<UserModel> = schema::users::table.load(&*db)?;
            for u in users.iter() {
                LibraryAccess::permit(&u, &lib, &*db)?;
            }
            Ok(lib)
        })
    }
}
