use std::sync::Mutex;
use lazy_static::lazy_static;
use rocket::serde::json::Json;
use rocket::time::OffsetDateTime;
use rocket_okapi::openapi;
use rocket_okapi::okapi::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};
use crate::{SERVER_LIMITS, SERVER_UPTIME};

lazy_static! {
    static ref SYS: Mutex<System> = Mutex::new(System::new());
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LimitsInfo {
    pub(crate) bytes: u64,
    pub(crate) data_form: u64,
    pub(crate) file: u64,
    pub(crate) form: u64,
    pub(crate) json: u64,
    pub(crate) msgpack: u64,
    pub(crate) string: u64,
}

impl LimitsInfo {
    pub(crate) fn from_limits(limits: &rocket::data::Limits) -> Self {
        let get = |key: &str| -> u64 { limits.get(key).map(u64::from).unwrap_or(0) };
        Self {
            bytes:     get("bytes"),
            data_form: get("data-form"),
            file:      get("file"),
            form:      get("form"),
            json:      get("json"),
            msgpack:   get("msgpack"),
            string:    get("string"),
        }
    }
}

#[openapi(tag = "Health Check")]
#[get("/ping")]
pub(crate) fn ping() -> &'static str {
    "pong"
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerStatsResponse {
    started: i64,
    limits: LimitsInfo,
    cpu_usage: f32,
    memory_usage: f32,
    disk_usage: f32,
}

#[openapi(tag = "Health Check")]
#[get("/stats")]
pub(crate) fn stats() -> Json<ServerStatsResponse> {
    let (cpu_usage, memory_usage) = {
        let mut sys = SYS.lock().unwrap();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let cpu = sys.global_cpu_usage();
        let mem_total = sys.total_memory();
        let mem = if mem_total > 0 {
            sys.used_memory() as f32 / mem_total as f32 * 100.0
        } else {
            0.0
        };
        (cpu, mem)
    };

    let disks = Disks::new_with_refreshed_list();
    let disk_total: u64 = disks.list().iter().map(|d| d.total_space()).sum();
    let disk_avail: u64 = disks.list().iter().map(|d| d.available_space()).sum();
    let disk_usage = if disk_total > 0 {
        (disk_total - disk_avail) as f32 / disk_total as f32 * 100.0
    } else {
        0.0
    };

    Json(ServerStatsResponse {
        started: SERVER_UPTIME.unix_timestamp(),
        limits: SERVER_LIMITS.clone(),
        cpu_usage,
        memory_usage,
        disk_usage,
    })
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpTimeResponse {
    seconds: i64,
}

#[openapi(tag = "Health Check")]
#[get("/uptime")]
pub(crate) fn uptime() -> Json<UpTimeResponse> {
    let seconds = (OffsetDateTime::now_utc() - *SERVER_UPTIME).whole_seconds();
    Json(UpTimeResponse { seconds })
}
