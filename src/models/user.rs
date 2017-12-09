use uuid;
use helpers::uuid::Uuid;
use chrono::NaiveDateTime;
use argon2rs::{verifier, Argon2};
use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use diesel::expression::exists;
use models::audiobook::Audiobook;
use models::library::{Library, LibraryAccess};
use std::result::Result as StdResult;
use diesel;
use diesel::result::QueryResult;
use base64;
use ring::rand::{SystemRandom, SecureRandom};

use schema::{users, api_tokens};
use schema;
use helpers::db::DB;

#[derive(Identifiable, Debug, Serialize, Deserialize, Queryable)]
#[table_name="users"]
#[hasmany(library_permissions)]
pub struct User {
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

error_chain! {
    foreign_links {
        Db(diesel::result::Error);
        UuidParse(uuid::ParseError);
    }

    errors {
        UserExists(user: String) {
            description("User already exists."),
            display("User {} already exists", user)
        }
    }
}

impl User {
    pub fn make_password_hash(new_password: &AsRef<str>) -> String {
        let rand = SystemRandom::new();
        let mut salt: [u8; 10] = [0; 10];
        rand.fill(&mut salt[..]);
        let session = verifier::Encoded::default2i(
            &new_password.as_ref().as_bytes(),
            &salt,
            &[],
            &[]
        );
        base64::encode(&session.to_u8())
    }

    pub fn accessible_libraries(&self, conn: &SqliteConnection) -> Result<Vec<Library>> {
        use diesel::expression::sql_literal::*;
        use diesel::types::*;
        use schema::libraries::dsl::libraries;
        use schema::library_permissions::dsl::{library_permissions, user_id};
        use schema::libraries::all_columns;

        Ok(library_permissions.inner_join(libraries).filter(user_id.eq(self.id))
            .select(all_columns)
            .get_results::<Library>(&*conn)?)
    }

    pub fn accessible_audiobooks(&self, conn: &SqliteConnection)
                -> QueryResult<Vec<Audiobook>> {
        use diesel::expression::sql_literal::*;
        use diesel::types::*;
        use schema::library_permissions::dsl::{library_permissions, user_id as library_permissions_user_id};
        use schema::libraries::dsl::{libraries, id};
        use schema::audiobooks::dsl::{audiobooks, library_id, deleted};
        use schema::audiobooks::all_columns;
        use schema::users::dsl::users;

        audiobooks.inner_join(
            libraries.inner_join(library_permissions))
            .filter(deleted.eq(false))
            .filter(library_permissions_user_id.eq(self.id))
            .select(all_columns)
            .get_results::<Audiobook>(&*conn)
    }

    pub fn create(email: &AsRef<str>, password: &AsRef<str>, conn: &SqliteConnection) -> Result<User> {
        use schema::users;
        use schema::users::dsl;
        let new_password_hash = User::make_password_hash(password);
        let results = dsl::users.filter(dsl::email.eq(email.as_ref().clone()))
            .first::<User>(&*conn);
        if results.is_ok() {
            return Err(ErrorKind::UserExists(email.as_ref().to_owned()).into());
        }
        conn.transaction(|| -> _ {
            let u = diesel::insert_into(users::table).values(&NewUser {
                email: email.as_ref().to_owned(),
                password_hash: new_password_hash,
            }).get_result::<User>(&*conn)?;
            let libraries: Vec<Library> = schema::libraries::table.load(&*conn)?;
            for l in libraries.iter() {
                LibraryAccess::permit(&u, &l, &*conn)?;
            }
            Ok(u)
        }).map_err(|e| ErrorKind::Db(e).into())
    }

    pub fn verify_password(&self, candidate_password: &str) -> bool {
        let data = base64::decode(&self.password_hash).expect("Malformed hash");
        let session = verifier::Encoded::from_u8(
            &data
        ).expect("Cant load hashing setting.");
        session.verify(candidate_password.as_bytes())
    }

    pub fn generate_api_token(&self, db: DB) -> Result<ApiToken> {
        let new_token = NewApiToken {
            user_id: self.id
        };
        let token = diesel::insert_into(api_tokens::table)
            .values(&new_token)
            .get_result::<ApiToken>(&*db)?;

        Ok(token)
    }

    pub fn get_user_from_api_token(token_id_string: &str, db: &SqliteConnection) -> Result<Option<User>> {
        use schema;
        use schema::api_tokens::dsl::*;

        use schema::users::dsl::*;

        let token_id = Uuid::parse_str(token_id_string)?;
        if let Some(token) = api_tokens.filter(schema::api_tokens::dsl::id.eq(token_id)).first::<ApiToken>(&*db).optional()? {
            Ok(users.filter(schema::users::dsl::id.eq(token.user_id)).first::<User>(&*db).optional()?)
        } else {
            Ok(None)
        }
    }

    pub fn get_book_if_accessible(self, book_id: &Uuid, conn: &SqliteConnection) -> QueryResult<Option<Audiobook>> {
        use diesel::expression::sql_literal::*;
        use diesel::types::*;
        use schema::library_permissions::dsl::{library_permissions, user_id as library_permissions_user_id};
        use schema::audiobooks::dsl::{audiobooks, id as audiobook_id};
        use schema::audiobooks::all_columns;
        use schema::libraries::dsl::libraries;

        Ok(audiobooks.inner_join(
                libraries.inner_join(library_permissions)
            )
            .filter(library_permissions_user_id.eq(self.id))
            .filter(audiobook_id.eq(book_id))
            .select(all_columns)
            .get_result::<Audiobook>(&*conn).optional()?)
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

#[derive(Debug, Queryable, Serialize)]
#[table_name="api_tokens"]
pub struct ApiToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: NaiveDateTime,
}
