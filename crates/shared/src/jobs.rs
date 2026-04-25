use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display};
use schemars::JsonSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, AsRefStr, Display, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum JobType {
    Full,
    DepthOnly,
    Compose,
    DepthCompose,
}

impl JobType {
    pub fn token_cost(&self) -> i32 {
        match self {
            JobType::Full => 100,
            JobType::DepthOnly => 30,
            JobType::Compose => 20,
            JobType::DepthCompose => 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, AsRefStr, Display, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Processing,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct JobMessage {
    pub job_id: uuid::Uuid,
    pub job_type: JobType,
    pub user_id: uuid::Uuid,
    pub options: JobOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct JobOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_disparity: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizontal_fov: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_mm: Option<f32>,
}
