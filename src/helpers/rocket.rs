use rocket::{self, Rocket};
use ::api;
use ::handlers;

pub fn factory(pool: super::db::Pool) -> Rocket {
    rocket::ignite()
        .manage(pool)
        .mount("/api/", routes![
            api::libraries::libraries,
            api::libraries::all_the_things,
            api::libraries::update_playstate,
        ])
        .mount("/api/auth/", routes![
               api::auth::login,
               api::auth::register,
               api::auth::whoami,
        ])
        .catch(errors![
            handlers::bad_request_handler,
            handlers::unauthorized_handler,
            handlers::forbidden_handler,
            handlers::not_found_handler,
            handlers::internal_server_error_handler,
            handlers::service_unavailable_handler
        ])
}
