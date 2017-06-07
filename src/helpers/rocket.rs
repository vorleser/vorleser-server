use rocket::{self, Rocket};
use ::api;
use ::handlers;

pub fn factory(pool: super::db::Pool) -> Rocket {
    rocket::ignite()
        .manage(pool)
        .mount("/api/", routes![
            api::libraries::libraries,
            api::libraries::all_the_things,
            api::audiobooks::update_playstate,
            api::audiobooks::audiobook,
        ])
        .mount("/api/auth/", routes![
               api::auth::login,
               api::auth::register,
               api::auth::whoami,
        ])
}
