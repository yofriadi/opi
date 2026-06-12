use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("No OpenAI Codex credentials found. Run `opi login` first.")]
    NotLoggedIn,
    #[error("Malformed credentials file. Please run `opi login` to re-authenticate.")]
    MalformedCredentials(#[source] serde_json::Error),
    #[error("I/O error reading/writing credentials: {0}")]
    Io(#[from] std::io::Error),
    #[error("JWT error: {0}")]
    Jwt(String),
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Authentication failed: {0}")]
    Auth(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFile {
    #[serde(rename = "openai-codex")]
    pub openai_codex: Option<AuthConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthConfig {
    #[serde(rename = "type")]
    pub auth_type: String, // "oauth"
    pub oauth: OauthPayload,
    pub issuer: String,
    #[serde(rename = "clientId")]
    pub client_id: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OauthPayload {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
    #[serde(rename = "idToken")]
    pub id_token: Option<String>,
    #[serde(rename = "expiresAt")]
    pub expires_at: i64,
    #[serde(rename = "accountId")]
    pub account_id: String,
}

impl std::fmt::Debug for OauthPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OauthPayload")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("id_token", &self.id_token.as_ref().map(|_| "[REDACTED]"))
            .field("expires_at", &self.expires_at)
            .field("account_id", &self.account_id)
            .finish()
    }
}

pub fn auth_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OPI_AUTH_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(windows) {
        std::env::var("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("opi").join("auth"))
            .unwrap_or_else(|_| PathBuf::from(".opi").join("auth"))
    } else {
        std::env::var("HOME")
            .map(|h| {
                PathBuf::from(h)
                    .join(".local")
                    .join("share")
                    .join("opi")
                    .join("auth")
            })
            .unwrap_or_else(|_| PathBuf::from(".opi").join("auth"))
    }
}

pub fn auth_file_path() -> PathBuf {
    auth_dir().join("auth.json")
}

pub fn auth_lock_path() -> PathBuf {
    auth_dir().join("auth.json.lock")
}

pub fn extract_claims(token: &str) -> Result<(String, i64), String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return Err("Invalid JWT: expected at least two dot-separated parts".to_string());
    }
    let payload_b64 = parts[1];

    use base64::Engine;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| format!("Failed to decode JWT base64: {e}"))?;
    let claims: serde_json::Value = serde_json::from_slice(&payload)
        .map_err(|e| format!("Failed to parse JWT claims JSON: {e}"))?;

    let account_id = if let Some(id) = claims.get("chatgpt_account_id").and_then(|v| v.as_str()) {
        Some(id.to_string())
    } else if let Some(auth) = claims.get("https://api.openai.com/auth") {
        auth.get("chatgpt_account_id")
            .and_then(|v| v.as_str())
            .map(|id| id.to_string())
    } else if let Some(orgs) = claims.get("organizations").and_then(|v| v.as_array()) {
        orgs.first()
            .and_then(|o| o.get("id").and_then(|v| v.as_str()))
            .map(|id| id.to_string())
    } else {
        None
    };

    let account_id = account_id.ok_or_else(|| {
        "Failed to extract chatgpt_account_id or organizations[0].id from JWT claims".to_string()
    })?;

    let exp = claims
        .get("exp")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "Failed to extract exp from JWT claims".to_string())?;

    Ok((account_id, exp))
}

#[cfg(unix)]
fn create_dir_all_secure(path: &std::path::Path) -> std::io::Result<()> {
    use std::fs::DirBuilder;
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    builder.mode(0o700);
    builder.create(path)
}

#[cfg(not(unix))]
fn create_dir_all_secure(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

#[cfg(unix)]
fn open_file_secure(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_file_secure(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
}

#[cfg(unix)]
fn fsync_parent_dir(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let dir = std::fs::File::open(parent)?;
        dir.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

pub fn load_auth() -> Result<Option<AuthConfig>, AuthError> {
    let path = auth_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let auth_file: AuthFile =
        serde_json::from_str(&content).map_err(AuthError::MalformedCredentials)?;
    Ok(auth_file.openai_codex)
}

pub fn save_auth(config: &AuthConfig) -> Result<(), AuthError> {
    let dir = auth_dir();
    create_dir_all_secure(&dir)?;

    let path = auth_file_path();
    let mut auth_file = if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or(AuthFile { openai_codex: None })
        } else {
            AuthFile { openai_codex: None }
        }
    } else {
        AuthFile { openai_codex: None }
    };

    auth_file.openai_codex = Some(config.clone());

    let content =
        serde_json::to_string_pretty(&auth_file).map_err(AuthError::MalformedCredentials)?;

    let tmp_path = dir.join("auth.json.tmp");

    {
        use std::io::Write;
        let mut file = open_file_secure(&tmp_path)?;

        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }

    std::fs::rename(&tmp_path, &path)?;
    fsync_parent_dir(&path)?;

    Ok(())
}

pub fn delete_auth() -> Result<(), AuthError> {
    let path = auth_file_path();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn make_jwt(payload_val: serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_string(&payload_val).unwrap());
        format!("{}.{}.signature", header, payload)
    }

    #[test]
    fn test_extract_claims_direct() {
        let jwt = make_jwt(serde_json::json!({
            "chatgpt_account_id": "acc-direct",
            "exp": 123456789i64
        }));
        let (account_id, exp) = extract_claims(&jwt).unwrap();
        assert_eq!(account_id, "acc-direct");
        assert_eq!(exp, 123456789);
    }

    #[test]
    fn test_extract_claims_nested() {
        let jwt = make_jwt(serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acc-nested"
            },
            "exp": 123456789i64
        }));
        let (account_id, exp) = extract_claims(&jwt).unwrap();
        assert_eq!(account_id, "acc-nested");
        assert_eq!(exp, 123456789);
    }

    #[test]
    fn test_extract_claims_organizations() {
        let jwt = make_jwt(serde_json::json!({
            "organizations": [
                {
                    "id": "acc-org"
                }
            ],
            "exp": 123456789i64
        }));
        let (account_id, exp) = extract_claims(&jwt).unwrap();
        assert_eq!(account_id, "acc-org");
        assert_eq!(exp, 123456789);
    }
}
