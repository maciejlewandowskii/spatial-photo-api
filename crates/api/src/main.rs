#[macro_use]
extern crate rocket;

use lazy_static::lazy_static;
use lambda_web::{is_running_on_lambda, launch_rocket_on_lambda, LambdaError};
use rocket::time::OffsetDateTime;

use crate::api::health_check::LimitsInfo;
use crate::docs::scalar_docs_ui::scalar_docs_ui;

mod api;
mod docs;

lazy_static! {
    static ref SERVER_UPTIME: OffsetDateTime = OffsetDateTime::now_utc();
    static ref SERVER_LIMITS: LimitsInfo = rocket::Config::figment()
        .extract::<rocket::Config>()
        .map(|c| LimitsInfo::from_limits(&c.limits))
        .unwrap_or_default();
}

#[rocket::main]
async fn main() -> Result<(), LambdaError> {
    let _ = *SERVER_UPTIME;
    let _ = &*SERVER_LIMITS;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting server");

    let rocket = rocket::build()
        .mount("/", api::routes())
        .mount("/docs", routes![scalar_docs_ui]);

    if is_running_on_lambda() {
        launch_rocket_on_lambda(rocket).await?;
    } else {
        rocket.launch().await.expect("server launch failed");
    }

    Ok(())
}
