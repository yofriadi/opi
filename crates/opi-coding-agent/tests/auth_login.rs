use base64::Engine;
use serial_test::serial;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn create_test_jwt(account_id: &str, expires_at: i64) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload_val = serde_json::json!({
        "chatgpt_account_id": account_id,
        "exp": expires_at
    });
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_string(&payload_val).unwrap());
    format!("{}.{}.signature", header, payload)
}

#[tokio::test]
#[serial]
async fn test_auth_lifecycle() {
    // Setup temp directory for auth files
    let tmp = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("OPI_AUTH_DIR", tmp.path());
        std::env::set_var("OPI_TEST_STATE", "test-state-abc");
        std::env::set_var("OPI_TEST_VERIFIER", "test-verifier-xyz");
    }
    let server = MockServer::start().await;

    // 1. Mock the token endpoint for browser login
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": create_test_jwt("user-123", 1999999999),
            "refresh_token": "refresh-token-123",
            "id_token": "id-token-123",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    // Spawn login_browser
    let server_uri = server.uri();
    let login_task = tokio::spawn(async move {
        opi_coding_agent::auth::login::login_browser(
            Some(&server_uri),
            Some("test-client-id"),
            None,
        )
        .await
    });

    // Wait for the TCP listener to bind
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let res = client
        .get("http://127.0.0.1:1455/auth/callback?code=mock-code-123&state=test-state-abc")
        .send()
        .await
        .unwrap();

    let status = res.status();
    let html = res.text().await.unwrap();
    println!("Redirect status: {}, body: {}", status, html);
    assert!(status.is_success());
    assert!(html.contains("Login Successful"));

    // Verify login browser finishes successfully
    let login_res = login_task.await.unwrap();
    assert!(login_res.is_ok());

    // 2. Verify login status
    let status_res = opi_coding_agent::auth::login::login_status();
    assert!(status_res.is_ok());

    // Verify file exists
    assert!(opi_coding_agent::auth::storage::auth_file_path().exists());

    // 3. Verify logout
    Mock::given(method("POST"))
        .and(path("/oauth/revoke"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let logout_res = opi_coding_agent::auth::login::logout(None).await;
    assert!(logout_res.is_ok());

    // Verify file deleted
    assert!(!opi_coding_agent::auth::storage::auth_file_path().exists());

    // 4. Verify device flow login
    // Mock usercode request
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/usercode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "device_auth_id": "device-id-123",
            "user_code": "ABCD-1234",
            "interval": 1
        })))
        .mount(&server)
        .await;

    // Mock device token poll
    Mock::given(method("POST"))
        .and(path("/api/accounts/deviceauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "authorization_code": "device-auth-code",
            "code_verifier": "device-code-verifier"
        })))
        .mount(&server)
        .await;

    // Mock token exchange for device code (which matches /oauth/token again)
    // Note: Since we remounted /oauth/token or it was already mounted, we can mount it again
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": create_test_jwt("device-user", 2999999999),
            "refresh_token": "device-refresh-token",
            "id_token": "device-id-token",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let device_login_res = opi_coding_agent::auth::login::login_device(
        Some(&server.uri()),
        Some("device-client-id"),
        None,
    )
    .await;

    assert!(device_login_res.is_ok());

    // Verify status shows device user
    let status_res = opi_coding_agent::auth::login::login_status();
    assert!(status_res.is_ok());

    unsafe {
        std::env::remove_var("OPI_AUTH_DIR");
        std::env::remove_var("OPI_TEST_STATE");
        std::env::remove_var("OPI_TEST_VERIFIER");
    }
}
