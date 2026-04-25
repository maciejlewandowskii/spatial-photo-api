use rocket::Route;
use rocket_okapi::openapi_get_routes;

pub(crate) mod auth;
pub(crate) mod health_check;
pub(crate) mod jobs;
pub(crate) mod tokens;
pub(crate) mod ui;

pub(crate) fn openapi_routes() -> Vec<Route> {
    openapi_get_routes![
        health_check::ping,
        health_check::stats,
        health_check::uptime
    ]
}

pub(crate) fn auth_routes() -> Vec<Route> {
    routes![
        auth::register_handler,
        auth::login_handler,
        auth::refresh_handler,
        auth::logout_handler,
        auth::logout_get,
        auth::me_handler,
        auth::create_key,
        auth::list_keys,
        auth::delete_key,
    ]
}

pub(crate) fn job_routes() -> Vec<Route> {
    jobs::routes()
}

pub(crate) fn token_routes() -> Vec<Route> {
    tokens::routes()
}

pub(crate) fn ui_routes() -> Vec<Route> {
    routes![
        ui::index,
        ui::login_page,
        ui::register_page,
        ui::dashboard,
        ui::job_list_html,
    ]
}
