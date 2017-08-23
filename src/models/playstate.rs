use uuid::Uuid;
use diesel;
use diesel::prelude::*;
use diesel::pg::PgConnection;
use schema::playstates;
use schema::library_permissions;
use chrono::NaiveDateTime;

#[derive(Insertable, Queryable, AsChangeset, Serialize, Deserialize, Debug)]
#[table_name="playstates"]
#[changeset_for(playstates, treat_none_as_null="true")]
pub struct Playstate {
    pub audiobook_id: Uuid,
    pub user_id: Uuid,
    pub position: f64,
    pub timestamp: NaiveDateTime,
}

impl Playstate {
    pub fn upsert(self, db: &PgConnection) -> Result<Playstate, diesel::result::Error> {
        use schema::playstates::dsl::*;
        use diesel::pg::upsert::*;
        diesel::insert(
            &self.on_conflict(
                (audiobook_id, user_id),
                do_update().set(&self)
            )
        ).into(playstates).get_result(&*db)
    }

    pub fn into_api_playstate(&self) -> ApiPlaystate {
        ApiPlaystate {
            audiobook_id: self.audiobook_id,
            position: self.position,
            // TODO: don't unwrap here
            timestamp: self.timestamp,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiPlaystate {
    pub audiobook_id: Uuid,
    pub position: f64,
    pub timestamp: NaiveDateTime,
}

use models::user::UserModel;

impl ApiPlaystate {
    pub fn into_playstate(&self, user: &UserModel) -> Playstate {
        Playstate {
            audiobook_id: self.audiobook_id,
            user_id: user.id,
            position: self.position,
            timestamp: self.timestamp,
        }
    }
}
