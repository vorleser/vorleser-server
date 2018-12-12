use diesel;
use diesel::prelude::*;
use crate::helpers::uuid::Uuid;
use crate::schema::{libraries, audiobooks, library_permissions, self};
use crate::models::audiobook::Audiobook;
use crate::models::library::Library;

use crate::helpers::db;
use crate::models::user::User;

#[table_name="library_permissions"]
#[belongs_to(User, foreign_key="user_id")]
#[belongs_to(Library, foreign_key="library_id")]
#[primary_key(library_id, user_id)]
#[derive(Debug, Clone, Queryable, Associations, Identifiable, Insertable)]
pub struct LibraryPermission {
    pub library_id: Uuid,
    pub user_id: Uuid,
}

impl LibraryPermission {
    pub fn permit(user: &User, library: &Library, db: &db::Connection) -> Result<Self, diesel::result::Error> {
        let permission = Self {
            library_id: library.id,
            user_id: user.id,
        };
        diesel::insert_into(library_permissions::table)
            .values(&permission).execute(&*db)?;
        Ok(permission)
    }
}

