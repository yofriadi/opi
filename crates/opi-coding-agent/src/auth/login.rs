use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct OauthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    pub expires_in: Option<i64>,
}

async fn exchange_code(
    client: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<OauthResponse, crate::auth::storage::AuthError> {
    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("code", code),
        ("code_verifier", code_verifier),
        ("redirect_uri", redirect_uri),
    ];
    let res = client.post(token_url).form(&params).send().await?;

    let status = res.status();
    let body = res.text().await?;
    if !status.is_success() {
        return Err(crate::auth::storage::AuthError::Auth(format!(
            "Token exchange failed (status {status}): {body}"
        )));
    }

    let token_res: OauthResponse = serde_json::from_str(&body)
        .map_err(crate::auth::storage::AuthError::MalformedCredentials)?;
    Ok(token_res)
}

pub async fn login_browser(
    issuer: Option<&str>,
    client_id: Option<&str>,
    proxy_config: Option<&crate::config::ProviderProxyConfig>,
) -> Result<(), crate::auth::storage::AuthError> {
    let client = match crate::config::build_http_client(proxy_config) {
        Ok(arc_client) => arc_client.client().clone(),
        Err(e) => return Err(crate::auth::storage::AuthError::Auth(e.to_string())),
    };
    let issuer = issuer.unwrap_or("https://auth.openai.com");
    let client_id = client_id.unwrap_or("app_EMoamEEZ73f0CkXaXp7hrann");

    let pkce = crate::auth::pkce::generate_pkce();

    let state_clone = pkce.state.clone();
    let server_handle =
        tokio::spawn(
            async move { crate::auth::callback::start_callback_server(state_clone).await },
        );

    let redirect_uri = "http://127.0.0.1:1455/auth/callback";
    let auth_url = format!(
        "{}/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}&scope=openid+profile+email+offline_access&id_token_add_organizations=true&codex_cli_simplified_flow=true&originator=pi",
        issuer, client_id, redirect_uri, pkce.code_challenge, pkce.state
    );

    println!("Open this URL to authorize:");
    println!("{}\n", auth_url);

    if let Err(e) = webbrowser::open(&auth_url) {
        println!("Could not open browser automatically: {e}");
    }

    println!("Waiting for browser redirect or manual entry.");
    println!(
        "If you are on a headless system or remote container, authorize in another browser and paste the callback URL or authorization code here: "
    );

    let stdin_handle = tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let stdin = tokio::io::stdin();
        let mut reader = tokio::io::BufReader::new(stdin);
        let mut line = String::new();
        if reader.read_line(&mut line).await.is_ok() {
            Some(line.trim().to_string())
        } else {
            None
        }
    });

    let code = tokio::select! {
        res = server_handle => {
            match res {
                Ok(Ok(cb_res)) => cb_res.code,
                Ok(Err(e)) => return Err(crate::auth::storage::AuthError::Auth(format!("Callback server error: {e}"))),
                Err(_) => return Err(crate::auth::storage::AuthError::Auth("Callback server task panicked".to_string())),
            }
        }
        res = stdin_handle => {
            match res {
                Ok(Some(input)) => {
                    if let Some(pos) = input.find("code=") {
                        let mut code_val = &input[pos + 5..];
                        if let Some(end_pos) = code_val.find('&') {
                            code_val = &code_val[..end_pos];
                        }
                        code_val.to_string()
                    } else {
                        input
                    }
                }
                _ => return Err(crate::auth::storage::AuthError::Auth("Failed to read from stdin".to_string())),
            }
        }
    };

    println!("Exchanging authorization code for tokens...");
    let token_url = format!("{}/oauth/token", issuer);
    let token_res = exchange_code(
        &client,
        &token_url,
        client_id,
        &code,
        &pkce.code_verifier,
        redirect_uri,
    )
    .await?;

    println!("Extracting token claims...");
    let (account_id, exp) = crate::auth::storage::extract_claims(&token_res.access_token)
        .map_err(crate::auth::storage::AuthError::Jwt)?;

    let auth_config = crate::auth::storage::AuthConfig {
        auth_type: "oauth".to_string(),
        oauth: crate::auth::storage::OauthPayload {
            access_token: token_res.access_token,
            refresh_token: token_res.refresh_token,
            id_token: token_res.id_token,
            expires_at: exp,
            account_id,
        },
        issuer: issuer.to_string(),
        client_id: client_id.to_string(),
    };

    crate::auth::storage::save_auth(&auth_config)?;

    println!("Login successful!");
    Ok(())
}

pub async fn login_device(
    issuer: Option<&str>,
    client_id: Option<&str>,
    proxy_config: Option<&crate::config::ProviderProxyConfig>,
) -> Result<(), crate::auth::storage::AuthError> {
    let issuer = issuer.unwrap_or("https://auth.openai.com");
    let client_id = client_id.unwrap_or("app_EMoamEEZ73f0CkXaXp7hrann");

    let client = match crate::config::build_http_client(proxy_config) {
        Ok(arc_client) => arc_client.client().clone(),
        Err(e) => return Err(crate::auth::storage::AuthError::Auth(e.to_string())),
    };
    let usercode_url = format!("{}/api/accounts/deviceauth/usercode", issuer);
    let res = client
        .post(&usercode_url)
        .json(&serde_json::json!({ "client_id": client_id }))
        .send()
        .await
        .map_err(crate::auth::storage::AuthError::Http)?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(crate::auth::storage::AuthError::Auth(format!(
            "Device code request failed (status {status}): {body}"
        )));
    }

    #[derive(Deserialize)]
    struct UsercodeResponse {
        device_auth_id: String,
        user_code: String,
        interval: serde_json::Value,
    }

    let usercode_res: UsercodeResponse = res
        .json()
        .await
        .map_err(crate::auth::storage::AuthError::Http)?;

    let mut interval_sec = match usercode_res.interval {
        serde_json::Value::Number(num) => num.as_u64().unwrap_or(5),
        serde_json::Value::String(s) => s.trim().parse::<u64>().unwrap_or(5),
        _ => 5,
    };

    let verification_url = format!("{}/deviceauth/callback", issuer);
    println!("Device authentication flow initiated.");
    println!("Please open the following URL in your browser to authorize:");
    println!("{}\n", verification_url);
    println!("Once the page loads, enter the following code:");
    println!("    {}\n", usercode_res.user_code);

    if let Err(e) = webbrowser::open(&verification_url) {
        println!("Could not open browser automatically: {e}");
    }

    println!("Waiting for device authorization (timeout: 15 minutes)...");

    let token_poll_url = format!("{}/api/accounts/deviceauth/token", issuer);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15 * 60);

    let mut auth_code = None;
    let mut code_verifier = None;

    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_secs(interval_sec)).await;

        let poll_res = client
            .post(&token_poll_url)
            .json(&serde_json::json!({
                "device_auth_id": usercode_res.device_auth_id,
                "user_code": usercode_res.user_code,
            }))
            .send()
            .await;

        match poll_res {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    #[derive(Deserialize)]
                    struct PollSuccessResponse {
                        authorization_code: String,
                        code_verifier: String,
                    }
                    if let Ok(success) = response.json::<PollSuccessResponse>().await {
                        auth_code = Some(success.authorization_code);
                        code_verifier = Some(success.code_verifier);
                        break;
                    } else {
                        return Err(crate::auth::storage::AuthError::Auth(
                            "Failed to parse poll success response".to_string(),
                        ));
                    }
                } else {
                    let body = response.text().await.unwrap_or_default();
                    if let Ok(err_val) = serde_json::from_str::<serde_json::Value>(&body) {
                        let err_type = err_val
                            .get("error")
                            .and_then(|e| e.get("type").and_then(|t| t.as_str()))
                            .unwrap_or("");
                        let err_msg = err_val
                            .get("error")
                            .and_then(|e| e.get("message").and_then(|m| m.as_str()))
                            .unwrap_or("");

                        if err_type == "authorization_pending"
                            || err_type == "invalid_request_error"
                            || err_msg.contains("pending")
                        {
                            continue;
                        } else if err_type == "slow_down" {
                            interval_sec += 5;
                            continue;
                        } else {
                            return Err(crate::auth::storage::AuthError::Auth(format!(
                                "Device auth failed: {err_msg} ({err_type})"
                            )));
                        }
                    } else {
                        return Err(crate::auth::storage::AuthError::Auth(format!(
                            "Poll request failed with status {status}: {body}"
                        )));
                    }
                }
            }
            Err(e) => {
                println!("Network error during polling: {e}. Retrying...");
                continue;
            }
        }
    }

    let auth_code = match auth_code {
        Some(c) => c,
        None => {
            return Err(crate::auth::storage::AuthError::Auth(
                "Device authentication timed out after 15 minutes".to_string(),
            ));
        }
    };
    let code_verifier = code_verifier.unwrap();

    println!("Device authorized. Exchanging authorization code for tokens...");
    let token_url = format!("{}/oauth/token", issuer);
    let token_res = exchange_code(
        &client,
        &token_url,
        client_id,
        &auth_code,
        &code_verifier,
        &verification_url,
    )
    .await?;

    println!("Extracting token claims...");
    let (account_id, exp) = crate::auth::storage::extract_claims(&token_res.access_token)
        .map_err(crate::auth::storage::AuthError::Jwt)?;

    let auth_config = crate::auth::storage::AuthConfig {
        auth_type: "oauth".to_string(),
        oauth: crate::auth::storage::OauthPayload {
            access_token: token_res.access_token,
            refresh_token: token_res.refresh_token,
            id_token: token_res.id_token,
            expires_at: exp,
            account_id,
        },
        issuer: issuer.to_string(),
        client_id: client_id.to_string(),
    };

    crate::auth::storage::save_auth(&auth_config)?;

    println!("Login successful!");
    Ok(())
}

pub async fn logout(
    proxy_config: Option<&crate::config::ProviderProxyConfig>,
) -> Result<(), crate::auth::storage::AuthError> {
    if let Ok(Some(auth_config)) = crate::auth::storage::load_auth() {
        let revoke_url = format!("{}/oauth/revoke", auth_config.issuer);
        let client = match crate::config::build_http_client(proxy_config) {
            Ok(arc_client) => arc_client.client().clone(),
            Err(e) => return Err(crate::auth::storage::AuthError::Auth(e.to_string())),
        };
        let params = [
            ("client_id", auth_config.client_id.as_str()),
            ("token", auth_config.oauth.refresh_token.as_str()),
            ("token_type_hint", "refresh_token"),
        ];
        let _ = client.post(&revoke_url).form(&params).send().await;
    }

    crate::auth::storage::delete_auth()?;

    println!("Logged out successfully.");
    Ok(())
}

pub fn login_status() -> Result<(), crate::auth::storage::AuthError> {
    match crate::auth::storage::load_auth()? {
        Some(auth_config) => {
            println!("Logged in to OpenAI Codex.");
            println!("Account ID: {}", auth_config.oauth.account_id);
            let expires_at = auth_config.oauth.expires_at;
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0) as i64;
            let remaining = expires_at - current_time;
            if remaining <= 0 {
                println!("Status: Expired (requires refresh)");
            } else {
                println!("Status: Active (expires in {} seconds)", remaining);
            }
            Ok(())
        }
        None => {
            println!("Not logged in. Run `opi login` to authenticate.");
            Err(crate::auth::storage::AuthError::NotLoggedIn)
        }
    }
}
