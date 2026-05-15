use rocket::Route;
use rocket_okapi::openapi_get_routes;

pub(crate) mod auth;
pub(crate) mod health_check;
pub(crate) mod htmx;
pub(crate) mod jobs;
pub(crate) mod tokens;
pub(crate) mod ui;

pub(crate) fn openapi_routes() -> Vec<Route> {
    openapi_get_routes![
        health_check::ping,
        health_check::stats,
        health_check::uptime,
        auth::register_handler,
        auth::login_handler,
        auth::refresh_handler,
        auth::logout_handler,
        auth::logout_get,
        auth::me_handler,
        auth::create_key,
        auth::list_keys,
        auth::delete_key,
        jobs::submit_convert::handler,
        jobs::submit_depth::handler,
        jobs::submit_compose::handler,
        jobs::submit_depth_compose::handler,
        jobs::get_job::handler,
        jobs::list_jobs::handler,
        jobs::cancel_job::handler,
        jobs::download_result::handler,
        tokens::balance::handler,
        tokens::history::handler,
        tokens::estimate::handler,
    ]
}

pub(crate) fn ui_routes() -> Vec<Route> {
    let mut routes = routes![
        ui::index,
        ui::login_page,
        ui::register_page,
        ui::dashboard,
        ui::job_list_html,
    ];
    routes.extend(htmx::routes());
    routes
}
