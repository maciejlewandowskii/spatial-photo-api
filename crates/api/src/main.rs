#[macro_use]
extern crate rocket;

use lambda_web::{is_running_on_lambda, launch_rocket_on_lambda, LambdaError};
use lazy_static::lazy_static;
use rocket::time::OffsetDateTime;
use shared::db::PgPool;

use crate::api::health_check::LimitsInfo;
use crate::api::jobs::{S3State, SqsState};
use crate::docs::scalar_docs_ui::scalar_docs_ui;
use crate::guards::JwtSecret;

mod api;
mod docs;
mod error;
mod guards;

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

    let db_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable must be set");
    let jwt_secret =
        std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");
    let s3_bucket =
        std::env::var("S3_BUCKET").expect("S3_BUCKET environment variable must be set");
    let queue_url =
        std::env::var("SQS_QUEUE_URL").expect("SQS_QUEUE_URL environment variable must be set");

    let pool = PgPool::connect(&db_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("../shared/migrations")
        .run(&pool)
        .await
        .expect("database migration failed");

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

    let rocket = rocket::build()
        .manage(pool)
        .manage(JwtSecret(jwt_secret))
        .manage(s3_state)
        .manage(sqs_state)
        .mount("/", api::openapi_routes())
        .mount("/auth", api::auth_routes())
        .mount("/jobs", api::job_routes())
        .mount("/docs", routes![scalar_docs_ui]);

    if is_running_on_lambda() {
        launch_rocket_on_lambda(rocket).await?;
    } else {
        rocket.launch().await.expect("server launch failed");
    }

    Ok(())
}
