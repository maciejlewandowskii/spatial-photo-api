use super::rocket;
use rocket::local::asynchronous::Client;
use rocket::http::{Status, ContentType};
use serde_json::json;

async fn test_client() -> Client {
    Client::tracked(rocket().await.expect("rocket build failed"))
        .await
        .expect("valid rocket instance")
}

#[tokio::test]
async fn test_ping() {
    let client = test_client().await;
    let response = client.get("/ping").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.into_string().await, Some("pong".into()));
}

#[tokio::test]
async fn test_stats() {
    let client = test_client().await;
    let response = client.get("/stats").dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    assert_eq!(response.content_type(), Some(ContentType::JSON));
}

#[tokio::test]
async fn test_register_and_login() {
    let client = test_client().await;
    let email = format!("test-{}@example.com", uuid::Uuid::new_v4());
    let password = "password123";

    // 1. Register
    let response = client.post("/register")
        .header(ContentType::JSON)
        .body(json!({
            "email": email,
            "password": password
        }).to_string())
        .dispatch().await;
    
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().await.expect("valid json response");
    assert!(body.get("access_token").is_some());
    let access_token = body["access_token"].as_str().unwrap().to_string();

    // 2. Get Profile
    let response = client.get("/me")
        .header(rocket::http::Header::new("Authorization", format!("Bearer {access_token}")))
        .dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().await.expect("valid json response");
    assert_eq!(body["email"], email);
    assert_eq!(body["tokens"], 1000);

    // 3. Login
    let response = client.post("/login")
        .header(ContentType::JSON)
        .body(json!({
            "email": email,
            "password": password
        }).to_string())
        .dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().await.expect("valid json response");
    assert!(body.get("access_token").is_some());
}

#[tokio::test]
async fn test_full_job_flow() {
    let client = test_client().await;
    let email = format!("test-job-{}@example.com", uuid::Uuid::new_v4());
    let password = "password123";

    // 1. Register
    let response = client.post("/register")
        .header(ContentType::JSON)
        .body(json!({"email": email, "password": password}).to_string())
        .dispatch().await;
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    let access_token = body["access_token"].as_str().unwrap().to_string();
    let auth_header = rocket::http::Header::new("Authorization", format!("Bearer {access_token}"));

    // 2. Check initial balance
    let response = client.get("/balance").header(auth_header.clone()).dispatch().await;
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    assert_eq!(body["tokens"], 1000);

    // 3. Submit a depth job
    let jpeg_data = vec![0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x08, 0x08, 0x0B, 0x0B, 0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32, 0x3C, 0x26, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x00, 0xFF, 0xD9];
    
    let response = client.post("/depth")
        .header(auth_header.clone())
        .body(jpeg_data)
        .dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    let job_id = body["id"].as_str().unwrap().to_string();
    assert_eq!(body["jobType"], "depth_only");
    assert_eq!(body["tokensCharged"], 30);

    // 4. Check balance after job
    let response = client.get("/balance").header(auth_header.clone()).dispatch().await;
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    assert_eq!(body["tokens"], 970);

    // 5. List jobs
    let response = client.get("/list").header(auth_header.clone()).dispatch().await;
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    assert_eq!(body["total"], 1);
    assert_eq!(body["jobs"][0]["id"], job_id);

    // 6. Get job details
    let response = client.get(format!("/{job_id}")).header(auth_header.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    assert_eq!(body["id"], job_id);
    assert_eq!(body["status"], "pending");
}

#[tokio::test]
async fn test_api_key_flow() {
    let client = test_client().await;
    let email = format!("test-key-{}@example.com", uuid::Uuid::new_v4());
    
    // 1. Register
    let response = client.post("/register")
        .header(ContentType::JSON)
        .body(json!({"email": email, "password": "password123"}).to_string())
        .dispatch().await;
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    let access_token = body["access_token"].as_str().unwrap().to_string();
    let auth_header = rocket::http::Header::new("Authorization", format!("Bearer {access_token}"));

    // 2. Create API Key
    let response = client.post("/keys")
        .header(auth_header)
        .header(ContentType::JSON)
        .body(json!({"name": "test-key"}).to_string())
        .dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    let api_key = body["key"].as_str().unwrap().to_string();
    let key_id = body["id"].as_str().unwrap().to_string();

    // 3. Use API Key to get profile
    let response = client.get("/me")
        .header(rocket::http::Header::new("X-API-Key", api_key.clone()))
        .dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let body: serde_json::Value = response.into_json().await.expect("valid json");
    assert_eq!(body["email"], email);

    // 4. Delete API Key
    let response = client.delete(format!("/keys/{key_id}"))
        .header(rocket::http::Header::new("X-API-Key", api_key.clone()))
        .dispatch().await;
    assert_eq!(response.status(), Status::NoContent);

    // 5. Verify key is revoked
    let response = client.get("/me")
        .header(rocket::http::Header::new("X-API-Key", api_key))
        .dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}
