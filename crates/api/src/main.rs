#[macro_use]
extern crate rocket;

use lambda_web::{is_running_on_lambda, launch_rocket_on_lambda, LambdaError};
use lazy_static::lazy_static;
use rocket::time::OffsetDateTime;
use rocket_dyn_templates::Template;
use shared::db::PgPool;

use crate::api::health_check::LimitsInfo;
use crate::api::jobs::{S3State, SqsState};
use crate::docs::scalar_docs_ui::scalar_docs_ui;
use crate::guards::JwtSecret;

mod api;
mod docs;
mod error;
mod guards;

#[cfg(test)]
mod tests;

lazy_static! {
    static ref SERVER_UPTIME: OffsetDateTime = OffsetDateTime::now_utc();
    static ref SERVER_LIMITS: LimitsInfo = rocket::Config::figment()
        .extract::<rocket::Config>()
        .map(|c| LimitsInfo::from_limits(&c.limits))
        .unwrap_or_default();
}

#[rocket::main]
async fn main() -> Result<(), LambdaError> {
    let rocket = rocket().await?;

    if is_running_on_lambda() {
        launch_rocket_on_lambda(rocket).await?;
    } else {
        rocket.launch().await.map_err(|e| LambdaError::from(format!("server launch failed: {e}")))?;
    }

    Ok(())
}

pub async fn rocket() -> Result<rocket::Rocket<rocket::Build>, LambdaError> {
    dotenvy::dotenv().ok();

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        );

    if is_running_on_lambda() {
        subscriber.json().init();
    } else {
        let _ = subscriber.try_init();
    }

    let db_url =
        std::env::var("DATABASE_URL").map_err(|_| LambdaError::from("DATABASE_URL environment variable must be set"))?;
    let jwt_secret =
        std::env::var("JWT_SECRET").map_err(|_| LambdaError::from("JWT_SECRET environment variable must be set"))?;
    let s3_bucket =
        std::env::var("S3_BUCKET").map_err(|_| LambdaError::from("S3_BUCKET environment variable must be set"))?;
    let queue_url =
        std::env::var("SQS_QUEUE_URL").map_err(|_| LambdaError::from("SQS_QUEUE_URL environment variable must be set"))?;

    let pool = PgPool::connect(&db_url)
        .await
        .map_err(|e| LambdaError::from(format!("failed to connect to database: {e}")))?;

    sqlx::migrate!("../shared/migrations")
        .run(&pool)
        .await
        .map_err(|e| LambdaError::from(format!("database migration failed: {e}")))?;

    tracing::info!("database ready");

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let s3_state = S3State {
        client: aws_sdk_s3::Client::new(&aws_config),
        bucket: s3_bucket,
    };
    let sqs_state = SqsState {
        client: aws_sdk_sqs::Client::new(&aws_config),
        queue_url,
    };

    let mut figment = rocket::Config::figment();

    if !is_running_on_lambda() {
        if std::path::Path::new("crates/api/templates").exists() {
            figment = figment.merge(("template_dir", "crates/api/templates"));
        } else if std::path::Path::new("templates").exists() {
            figment = figment.merge(("template_dir", "templates"));
        }
    }

    Ok(rocket::custom(figment)
        .manage(pool)
        .manage(JwtSecret(jwt_secret))
        .manage(s3_state)
        .manage(sqs_state)
        .attach(Template::fairing())
        .mount("/", api::ui_routes())
        .mount("/", api::openapi_routes())
        .mount("/docs", routes![scalar_docs_ui]))
}
