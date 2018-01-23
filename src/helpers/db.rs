use std::ops::Deref;
use rocket::http::Status;
use rocket::{Request, State, Outcome};
use rocket::request::{self, FromRequest};
use diesel::sqlite::SqliteConnection;
use r2d2_diesel::ConnectionManager;
use r2d2::{self, CustomizeConnection};
use std::env;
use diesel::dsl::sql;
use diesel::{self, ExecuteDsl};

pub type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub type PooledConnection = r2d2::PooledConnection<ConnectionManager<SqliteConnection>>;
pub type Connection = SqliteConnection;

/// Our connection_customizer will set timeout behavior on the SQLite connection
#[derive(Copy, Clone, Debug)]
pub struct BusyWaitConnectionCustomizer;

impl<C: diesel::Connection, E> CustomizeConnection<C, E> for BusyWaitConnectionCustomizer {
    fn on_acquire(&self, conn: &mut C) -> Result<(), E> {
        conn.batch_execute("PRAGMA busy_timeout = 1000;").unwrap();
        conn.batch_execute("PRAGMA journal_mode = WAL;").unwrap();
        Ok(())
    }
}

/// Initialize database DB pool from specified URL.
pub fn init_db_pool(url: String) -> Pool {
    init_db_pool_with_count(url, 10)
}

fn init_db_pool_with_count(url: String, count: u32) -> Pool {
    let manager = ConnectionManager::<SqliteConnection>::new(url);
    r2d2::Pool::builder()
        .connection_customizer(Box::new(BusyWaitConnectionCustomizer{}))
        .max_size(count)
        .build(manager)
        .expect("Failed to create pool.")
}

#[cfg(test)]
pub fn init_test_db_pool() -> Pool {
    use diesel::Connection;
    let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
    let pool = r2d2::Pool::builder()
        .connection_customizer(Box::new(BusyWaitConnectionCustomizer{}))
        .max_size(1)
        .build(manager)
        .expect("Failed to create pool.");
    ::embedded_migrations::run(&*pool.get().unwrap());
    (&*pool.get().unwrap()).begin_test_transaction();
    pool
}

/// Initializes a SQLite file, running the migrations and setting the journal mode.
pub fn init_db(url: String) {
    info!("Initializing database at {}", url);
    let pool = init_db_pool_with_count(url, 1);
    ::embedded_migrations::run(&*pool.get().unwrap());
}

pub struct DB(PooledConnection);

impl Deref for DB {
    type Target = SqliteConnection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for DB {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<DB, ()> {
        let pool = match <State<Pool> as FromRequest>::from_request(request) {
            Outcome::Success(pool) => pool,
            Outcome::Failure(e) => return Outcome::Failure(e),
            Outcome::Forward(_) => return Outcome::Forward(()),
        };

        match pool.get() {
            Ok(conn) => Outcome::Success(DB(conn)),
            Err(_) => Outcome::Failure((Status::ServiceUnavailable, ()))
        }
    }
}
