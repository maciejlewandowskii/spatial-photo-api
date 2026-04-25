use rocket::Route;
use rocket_okapi::openapi_get_routes;

pub(crate) mod auth;
pub(crate) mod health_check;
pub(crate) mod jobs;
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
        auth::register,
        auth::login,
        auth::refresh,
        auth::logout,
        auth::logout_get,
        auth::me,
        auth::create_key,
        auth::list_keys,
        auth::delete_key,
    ]
}

pub(crate) fn job_routes() -> Vec<Route> {
    routes![
        jobs::submit_convert,
        jobs::submit_depth,
        jobs::submit_compose,
        jobs::submit_depth_compose,
        jobs::get_job,
        jobs::list_jobs,
        jobs::cancel_job,
        jobs::download_result,
        ui::job_list_html,
    ]
}

pub(crate) fn ui_routes() -> Vec<Route> {
    routes![
        ui::index,
        ui::login_page,
        ui::register_page,
        ui::dashboard,
    ]
}
