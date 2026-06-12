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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[serial]
async fn test_concurrent_refresh() {
    let tmp = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("OPI_AUTH_DIR", tmp.path());
    }

    let server = MockServer::start().await;

    // Save expired credentials (expiry 10 seconds in the past)
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let expired_at = current_time - 10;

    let initial_jwt = create_test_jwt("old-account-id", expired_at);
    let initial_auth = opi_coding_agent::auth::storage::AuthConfig {
        auth_type: "oauth".to_string(),
        oauth: opi_coding_agent::auth::storage::OauthPayload {
            access_token: initial_jwt,
            refresh_token: "refresh-token-xyz".to_string(),
            id_token: None,
            expires_at: expired_at,
            account_id: "old-account-id".to_string(),
        },
        issuer: server.uri(),
        client_id: "test-client-id".to_string(),
    };

    opi_coding_agent::auth::storage::save_auth(&initial_auth).unwrap();

    // Mock token endpoint: return refreshed token, expect EXACTLY 1 call!
    let new_jwt = create_test_jwt("new-account-id", current_time + 3600);
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": new_jwt,
            "refresh_token": "new-refresh-token-xyz",
            "id_token": serde_json::Value::Null,
            "expires_in": 3600
        })))
        .expect(1) // EXACTLY 1 call is load-bearing!
        .mount(&server)
        .await;

    // Spawn N concurrent refresh tasks
    let mut handles = Vec::new();
    for _ in 0..10 {
        let handle =
            tokio::spawn(async { opi_coding_agent::auth::refresh::get_valid_token(None).await });
        handles.push(handle);
    }

    // Wait for all to finish
    let mut results = Vec::new();
    for handle in handles {
        let res = handle.await.unwrap();
        results.push(res);
    }

    // All should succeed and return the new token and new account ID (proving stale account ID override!)
    for res in results {
        let (token, account_id) = res.unwrap();
        assert_eq!(token, new_jwt);
        assert_eq!(account_id, "new-account-id");
    }

    // Check stored credentials are updated
    let stored = opi_coding_agent::auth::storage::load_auth()
        .unwrap()
        .unwrap();
    assert_eq!(stored.oauth.access_token, new_jwt);
    assert_eq!(stored.oauth.account_id, "new-account-id");

    unsafe {
        std::env::remove_var("OPI_AUTH_DIR");
    }
}
