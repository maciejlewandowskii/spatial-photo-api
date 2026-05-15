use rocket::Route;

pub mod register;
pub mod login;
pub mod convert;

pub fn routes() -> Vec<Route> {
    routes![
        register::handler,
        login::handler,
        convert::handler,
    ]
}
