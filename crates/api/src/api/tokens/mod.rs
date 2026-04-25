use rocket::Route;

pub mod balance;
pub mod history;
pub mod estimate;

pub fn routes() -> Vec<Route> {
    routes![
        balance::handler,
        history::handler,
        estimate::handler,
    ]
}
