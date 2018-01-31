use diesel;
use diesel::prelude::*;
use helpers::uuid::Uuid;
use schema::{libraries, audiobooks, library_permissions, self};
use models::audiobook::Audiobook;
use models::library::Library;

use helpers::db;
use models::user::User;

#[table_name="library_permissions"]
#[primary_key(library_id, user_id)]
#[derive(Debug, Clone, Queryable, Associations, Identifiable, Insertable)]
pub struct LibraryPermission {
    pub library_id: Uuid,
    pub user_id: Uuid,
}

impl LibraryPermission {
    pub fn permit(user: &User, library: &Library, db: &db::Connection) -> Result<Self, diesel::result::Error> {
        let permission = Self {
            library_id: library.id.clone(),
            user_id: user.id.clone(),
        };
        diesel::insert_into(library_permissions::table)
            .values(&permission).execute(&*db)?;
        Ok(permission)
    }
}

