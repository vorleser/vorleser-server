use uuid::Uuid;
use diesel::prelude::*;
use schema::audiobooks;
use schema::playstates;

#[table_name="audiobooks"]
#[derive(Insertable)]
struct NewAudiobook {
    pub title: String,
    pub length: f64,
}

#[derive(Debug, Queryable)]
#[hasmany(chapters)]
#[table_name="audiobooks"]
struct Audiobook {
    id: Uuid,
    title: String,
    length: f64
}

#[derive(Insertable)]
#[table_name="playstates"]
struct NewPlaystate {
    pub position: f64,
    pub completed: bool,
    pub user_id: Uuid,
    pub audiobook_id: Uuid
}

#[derive(Insertable)]
#[table_name="playstates"]
struct Playstate {
    pub position: f64,
    pub completed: bool,
    pub user_id: Uuid,
    pub audiobook_id: Uuid
}
