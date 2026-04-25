use rocket::Route;
use rocket_okapi::openapi_get_routes;

pub(crate) mod health_check;

pub(crate) fn routes() -> Vec<Route> {
    openapi_get_routes![
        health_check::ping,
        health_check::stats,
        health_check::uptime
    ]
}
