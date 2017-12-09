use uuid::Uuid;
use diesel;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use schema::playstates;
use schema::library_permissions;
use chrono::prelude::*;

#[derive(Identifiable, Associations, Insertable, Queryable, AsChangeset, Serialize, Deserialize, Debug)]
#[primary_key(audiobook_id, user_id)]
#[table_name="playstates"]
#[changeset_for(playstates, treat_none_as_null="true")]
#[belongs_to(User, foreign_key="user_id")]
pub struct Playstate {
    pub audiobook_id: Uuid,
    pub user_id: Uuid,
    pub position: f64,
    pub timestamp: NaiveDateTime,
}

impl Playstate {
    pub fn upsert(self, db: &SqliteConnection) -> Result<Playstate, diesel::result::Error> {
        use schema::playstates::dsl::*;
        diesel::insert_into(playstates)
            .values(&self)
            .on_conflict((audiobook_id, user_id))
            .do_update()
            .set(&self)
            .get_result(&*db)
    }

    pub fn into_api_playstate(&self) -> ApiPlaystate {
        ApiPlaystate {
            audiobook_id: self.audiobook_id,
            position: self.position,
            timestamp: DateTime::<Utc>::from_utc(self.timestamp, Utc),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiPlaystate {
    pub audiobook_id: Uuid,
    pub position: f64,
    pub timestamp: DateTime<Utc>,
}

use models::user::User;

impl ApiPlaystate {
    pub fn into_playstate(&self, user: &User) -> Playstate {
        Playstate {
            audiobook_id: self.audiobook_id,
            user_id: user.id,
            position: self.position,
            timestamp: self.timestamp.naive_utc(),
        }
    }
}
