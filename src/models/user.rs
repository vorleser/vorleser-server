use uuid::Uuid;
use chrono::NaiveDateTime;
use argon2rs::argon2i_simple;
use diesel::pg::PgConnection;
use diesel::prelude::*;

use schema::{users, api_tokens};
use helpers::util;
use helpers::db::DB;
use diesel;

#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct UserModel {
    pub id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserLoginToken {
    user_id: Uuid,
}

impl UserModel {
    pub fn make_password_hash(new_password: &AsRef<str>) -> String {
        // TODO: proper salting!!!
        let password_hash = argon2i_simple(new_password.as_ref(), "loginsalt");
        String::from_utf8_lossy(&password_hash).into_owned()
    }

    pub fn create(email: &AsRef<str>, password: &AsRef<str>, conn: &PgConnection) -> Result<UserModel, diesel::result::Error> {
        let new_password_hash = UserModel::make_password_hash(password);
        let new_user = NewUser {
            email: email.as_ref().to_owned(),
            password_hash: new_password_hash,
        };

        diesel::insert(&new_user)
            .into(users::table)
            .get_result::<UserModel>(&*conn)
    }

    pub fn verify_password(&self, candidate_password: &str) -> bool {
        let candidate_password = argon2i_simple(candidate_password, "loginsalt");
        let candidate_password_string = String::from_utf8_lossy(&candidate_password);
        self.password_hash == candidate_password_string
    }

    pub fn generate_api_token(&self, db: DB) -> String {
        let new_token = NewApiToken {
            user_id: self.id
        };
        let token = diesel::insert(&new_token)
            .into(api_tokens::table)
            .get_result::<ApiToken>(&*db)
            .expect("Error saving new api token");

        token.id.to_string()
    }

    pub fn get_user_from_api_token(token_id_string: &str, db: &PgConnection) -> Option<UserModel> {
        use schema;
        use schema::api_tokens::dsl::*;

        use schema::users::dsl::*;

        let token_id = Uuid::parse_str(token_id_string);
        if token_id.is_err() {
            return None;
        }

        let token = api_tokens.filter(schema::api_tokens::dsl::id.eq(token_id.unwrap())).first::<ApiToken>(&*db);
        if token.is_err() {
            return None;
        }

        let user = users.filter(schema::users::dsl::id.eq(token.unwrap().user_id)).first::<UserModel>(&*db);
        if user.is_err() {
            return None;
        }

        Some(user.unwrap())
    }
}

#[derive(Insertable)]
#[table_name="users"]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
}

#[derive(Insertable)]
#[table_name="api_tokens"]
pub struct NewApiToken {
    pub user_id: Uuid,
}

#[derive(Debug, Queryable)]
#[table_name="api_tokens"]
pub struct ApiToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: NaiveDateTime,
}
