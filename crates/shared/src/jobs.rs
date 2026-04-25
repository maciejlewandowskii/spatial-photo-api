use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

    pub fn as_str(&self) -> &'static str {
        match self {
            JobType::Full => "full",
            JobType::DepthOnly => "depth_only",
            JobType::Compose => "compose",
            JobType::DepthCompose => "depth_compose",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Processing,
    Complete,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Processing => "processing",
            JobStatus::Complete => "complete",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }
}

/// Parameters sent in the SQS message to the worker.
#[derive(Debug, Serialize, Deserialize)]
pub struct JobMessage {
    pub job_id: uuid::Uuid,
    pub job_type: JobType,
    pub user_id: uuid::Uuid,
    pub options: JobOptions,
}

/// Per-job processing options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
