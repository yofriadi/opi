use fd_lock::RwLock;
use std::fs::OpenOptions;

pub async fn get_valid_token(
    proxy_config: Option<&crate::config::ProviderProxyConfig>,
) -> Result<(String, String), crate::auth::storage::AuthError> {
    // 1. Optimistic check
    let auth_config = match crate::auth::storage::load_auth()? {
        Some(cfg) => cfg,
        None => return Err(crate::auth::storage::AuthError::NotLoggedIn),
    };

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;

    if auth_config.oauth.expires_at > current_time + 60 {
        return Ok((auth_config.oauth.access_token, auth_config.oauth.account_id));
    }

    // 2. We need to refresh. Acquire advisory lock on auth.json.lock.
    let lock_dir = crate::auth::storage::auth_dir();
    if !lock_dir.exists() {
        let _ = std::fs::create_dir_all(&lock_dir);
    }
    let lock_path = crate::auth::storage::auth_lock_path();
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)?;

    let mut lock = RwLock::new(lock_file);
    let _guard =
        tokio::task::block_in_place(|| lock.write().map_err(crate::auth::storage::AuthError::Io))?;

    // 3. Re-read auth config under the lock
    let auth_config = match crate::auth::storage::load_auth()? {
        Some(cfg) => cfg,
        None => return Err(crate::auth::storage::AuthError::NotLoggedIn),
    };

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;

    if auth_config.oauth.expires_at > current_time + 60 {
        return Ok((auth_config.oauth.access_token, auth_config.oauth.account_id));
    }

    // 4. Perform refresh request
    println!("Refreshing OpenAI Codex access token...");
    let client = match crate::config::build_http_client(proxy_config) {
        Ok(arc_client) => arc_client.client().clone(),
        Err(e) => return Err(crate::auth::storage::AuthError::Auth(e.to_string())),
    };
    let token_url = format!("{}/oauth/token", auth_config.issuer);

    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", auth_config.client_id.as_str()),
        ("refresh_token", auth_config.oauth.refresh_token.as_str()),
    ];

    let res = client.post(&token_url).form(&params).send().await?;

    let status = res.status();
    let body = res.text().await?;

    if !status.is_success() {
        return Err(crate::auth::storage::AuthError::Auth(format!(
            "Token refresh failed (status {status}): {body}"
        )));
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: Option<String>,
        id_token: Option<String>,
        expires_in: Option<i64>,
    }

    let refresh_res: RefreshResponse = serde_json::from_str(&body)
        .map_err(crate::auth::storage::AuthError::MalformedCredentials)?;

    // Extract claims from the new access token
    let (account_id, exp) = crate::auth::storage::extract_claims(&refresh_res.access_token)
        .map_err(crate::auth::storage::AuthError::Jwt)?;

    let updated_config = crate::auth::storage::AuthConfig {
        auth_type: "oauth".to_string(),
        oauth: crate::auth::storage::OauthPayload {
            access_token: refresh_res.access_token.clone(),
            refresh_token: refresh_res
                .refresh_token
                .unwrap_or(auth_config.oauth.refresh_token),
            id_token: refresh_res.id_token.or(auth_config.oauth.id_token),
            expires_at: exp,
            account_id: account_id.clone(),
        },
        issuer: auth_config.issuer,
        client_id: auth_config.client_id,
    };

    crate::auth::storage::save_auth(&updated_config)?;

    Ok((
        updated_config.oauth.access_token,
        updated_config.oauth.account_id,
    ))
}
