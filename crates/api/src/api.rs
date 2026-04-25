use rocket::Route;
use rocket_okapi::openapi_get_routes;

pub(crate) mod auth;
pub(crate) mod health_check;

pub(crate) fn openapi_routes() -> Vec<Route> {
    openapi_get_routes![
        health_check::ping,
        health_check::stats,
        health_check::uptime
    ]
}

pub(crate) fn auth_routes() -> Vec<Route> {
    routes![
        auth::register,
        auth::login,
        auth::refresh,
        auth::logout,
        auth::me,
        auth::create_key,
        auth::list_keys,
        auth::delete_key,
    ]
}
