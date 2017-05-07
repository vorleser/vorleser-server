use uuid::Uuid;
use diesel::prelude::*;
use schema::audiobooks;
use schema::playstates;

#[table_name="audiobooks"]
#[derive(Insertable)]
pub struct NewAudiobook {
    pub title: String,
    pub location: String,
    pub length: f64,
    pub library_id: Uuid,
    pub hash: Vec<u8>
}

#[derive(Debug, Queryable)]
#[hasmany(chapters)]
#[belongs_to(Library)]
#[table_name="audiobooks"]
pub struct Audiobook {
    pub id: Uuid,
    pub title: String,
    pub location: String,
    pub length: f64,
    pub library_id: Uuid,
    pub hash: Vec<u8>
}

#[derive(Insertable)]
#[table_name="playstates"]
pub struct NewPlaystate {
    pub position: f64,
    pub completed: bool,
    pub user_id: Uuid,
    pub audiobook_id: Uuid
}

#[derive(Insertable)]
#[table_name="playstates"]
pub struct Playstate {
    pub position: f64,
    pub completed: bool,
    pub user_id: Uuid,
    pub audiobook_id: Uuid
}
