use uuid::Uuid;
use diesel;
use schema::playstates;
use schema::library_permissions;
use chrono::NaiveDateTime;

#[derive(Insertable, Queryable, AsChangeset, Serialize, Deserialize)]
#[table_name="playstates"]
pub struct Playstate {
    pub audiobook_id: Uuid,
    pub user_id: Uuid,
    pub completed: bool,
    pub position: f64,
    pub timestamp: NaiveDateTime,
}

