use rocket_dyn_templates::{context, Template};
use shared::db::{self, PgPool};
use crate::guards::AuthUser;

#[get("/")]
pub async fn index(user: Option<AuthUser>) -> Template {
    if let Some(user) = user {
        Template::render("dashboard", context! { user })
    } else {
        Template::render("index", context! { user: None::<AuthUser> })
    }
}

#[get("/login")]
pub async fn login_page() -> Template {
    Template::render("index", context! { user: None::<AuthUser> })
}

#[get("/register")]
pub async fn register_page() -> Template {
    Template::render("register", context! { user: None::<AuthUser> })
}

#[get("/dashboard")]
pub async fn dashboard(user: AuthUser) -> Template {
    Template::render("dashboard", context! { user })
}

#[get("/jobs/list/html")]
pub async fn job_list_html(user: AuthUser, pool: &rocket::State<PgPool>) -> Template {
    let jobs = db::job::list_for_user(pool, user.user_id, 20, 0).await.unwrap_or_default();
    Template::render("job_list_partial", context! { jobs })
}
