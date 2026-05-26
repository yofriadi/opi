//! Bedrock credential resolution (task 3.1).
//!
//! Precedence: explicit config > env vars > ~/.aws/credentials profile.
//! No live AWS calls.

use std::path::Path;

/// Resolved AWS credentials for Bedrock.
///
/// Custom Debug redacts secret_access_key and session_token.
#[derive(Clone)]
pub struct BedrockCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
    pub region: String,
}

impl std::fmt::Debug for BedrockCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BedrockCredentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"***")
            .field("session_token", &self.session_token.as_ref().map(|_| "***"))
            .field("region", &self.region)
            .finish()
    }
}

/// Source of resolved credentials (for diagnostics, never logged with secrets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    ExplicitConfig,
    Environment,
    ProfileFile,
}

/// Input parameters for credential resolution.
pub struct CredentialResolutionInput<'a> {
    pub config_access_key_id: Option<&'a str>,
    pub config_secret_access_key: Option<&'a str>,
    pub config_session_token: Option<&'a str>,
    pub config_region: Option<&'a str>,
    pub env_access_key_id: Option<&'a str>,
    pub env_secret_access_key: Option<&'a str>,
    pub env_session_token: Option<&'a str>,
    pub env_region: Option<&'a str>,
    pub profile_name: Option<&'a str>,
    pub credentials_file_path: Option<&'a Path>,
}

impl<'a> CredentialResolutionInput<'a> {
    /// Build from environment variables.
    ///
    /// The caller must own the strings and pass references:
    /// ```
    /// let (akid, sak, token, region) = credentials_from_env();
    /// let input = CredentialResolutionInput::from_env_refs(
    ///     akid.as_deref(), sak.as_deref(), token.as_deref(), region.as_deref(),
    /// );
    /// ```
    pub fn from_env_refs(
        env_access_key_id: Option<&'a str>,
        env_secret_access_key: Option<&'a str>,
        env_session_token: Option<&'a str>,
        env_region: Option<&'a str>,
    ) -> Self {
        Self {
            config_access_key_id: None,
            config_secret_access_key: None,
            config_session_token: None,
            config_region: None,
            env_access_key_id,
            env_secret_access_key,
            env_session_token,
            env_region,
            profile_name: None,
            credentials_file_path: None,
        }
    }
}

/// Resolve Bedrock credentials with precedence:
/// explicit config > env vars > profile file.
///
/// Returns `None` when no credentials are found.
pub fn resolve_credentials(
    input: &CredentialResolutionInput<'_>,
) -> Option<(BedrockCredentials, CredentialSource)> {
    // 1. Explicit config takes highest precedence
    if let (Some(akid), Some(sak)) = (input.config_access_key_id, input.config_secret_access_key)
        && !akid.is_empty()
        && !sak.is_empty()
    {
        return Some((
            BedrockCredentials {
                access_key_id: akid.to_string(),
                secret_access_key: sak.to_string(),
                session_token: input
                    .config_session_token
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                region: input
                    .config_region
                    .filter(|s| !s.is_empty())
                    .unwrap_or("us-east-1")
                    .to_string(),
            },
            CredentialSource::ExplicitConfig,
        ));
    }

    // 2. Environment variables
    if let (Some(akid), Some(sak)) = (input.env_access_key_id, input.env_secret_access_key)
        && !akid.is_empty()
        && !sak.is_empty()
    {
        return Some((
            BedrockCredentials {
                access_key_id: akid.to_string(),
                secret_access_key: sak.to_string(),
                session_token: input
                    .env_session_token
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                region: input
                    .env_region
                    .filter(|s| !s.is_empty())
                    .unwrap_or("us-east-1")
                    .to_string(),
            },
            CredentialSource::Environment,
        ));
    }

    // 3. Profile file (~/.aws/credentials)
    if let (Some(profile), Some(path)) = (input.profile_name, input.credentials_file_path)
        && !profile.is_empty()
        && path.exists()
        && let Some(creds) = read_profile(path, profile)
    {
        return Some((creds, CredentialSource::ProfileFile));
    }

    None
}

/// Read a specific profile from an AWS credentials INI file.
pub fn read_profile(path: &Path, profile_name: &str) -> Option<BedrockCredentials> {
    let contents = std::fs::read_to_string(path).ok()?;
    let target_header = format!("[{profile_name}]");

    let mut in_target = false;
    let mut akid: Option<String> = None;
    let mut sak: Option<String> = None;
    let mut token: Option<String> = None;
    let mut region: Option<String> = None;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            if in_target {
                break;
            }
            in_target = line == target_header;
            continue;
        }
        if in_target && let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().to_string();
            match key {
                "aws_access_key_id" => akid = Some(value),
                "aws_secret_access_key" => sak = Some(value),
                "aws_session_token" => token = Some(value),
                "region" => region = Some(value),
                _ => {}
            }
        }
    }

    match (akid, sak) {
        (Some(a), Some(s)) => Some(BedrockCredentials {
            access_key_id: a,
            secret_access_key: s,
            session_token: token,
            region: region.unwrap_or_else(|| "us-east-1".to_string()),
        }),
        _ => None,
    }
}

/// Read AWS credentials from environment variables.
/// Returns (access_key_id, secret_access_key, session_token, region).
pub fn credentials_from_env() -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let akid = std::env::var("AWS_ACCESS_KEY_ID").ok();
    let sak = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
    let token = std::env::var("AWS_SESSION_TOKEN").ok();
    let region = std::env::var("AWS_REGION")
        .ok()
        .or_else(|| std::env::var("AWS_DEFAULT_REGION").ok());
    (akid, sak, token, region)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;

    #[allow(clippy::too_many_arguments)]
    fn input<'a>(
        config_akid: Option<&'a str>,
        config_sak: Option<&'a str>,
        config_token: Option<&'a str>,
        config_region: Option<&'a str>,
        env_akid: Option<&'a str>,
        env_sak: Option<&'a str>,
        env_token: Option<&'a str>,
        env_region: Option<&'a str>,
        profile: Option<&'a str>,
        creds_path: Option<&'a Path>,
    ) -> CredentialResolutionInput<'a> {
        CredentialResolutionInput {
            config_access_key_id: config_akid,
            config_secret_access_key: config_sak,
            config_session_token: config_token,
            config_region,
            env_access_key_id: env_akid,
            env_secret_access_key: env_sak,
            env_session_token: env_token,
            env_region,
            profile_name: profile,
            credentials_file_path: creds_path,
        }
    }

    #[test]
    fn explicit_config_takes_precedence() {
        let inp = input(
            Some("CONFIG_AKID"),
            Some("CONFIG_SAK"),
            Some("CONFIG_TOKEN"),
            Some("eu-west-1"),
            Some("ENV_AKID"),
            Some("ENV_SAK"),
            None,
            Some("us-west-2"),
            None,
            None,
        );
        let (result, source) = resolve_credentials(&inp).unwrap();
        assert_eq!(result.access_key_id, "CONFIG_AKID");
        assert_eq!(result.secret_access_key, "CONFIG_SAK");
        assert_eq!(result.session_token.as_deref(), Some("CONFIG_TOKEN"));
        assert_eq!(result.region, "eu-west-1");
        assert_eq!(source, CredentialSource::ExplicitConfig);
    }

    #[test]
    fn env_vars_when_no_config() {
        let inp = input(
            None,
            None,
            None,
            None,
            Some("ENV_AKID"),
            Some("ENV_SAK"),
            Some("ENV_TOKEN"),
            Some("ap-southeast-1"),
            None,
            None,
        );
        let (result, source) = resolve_credentials(&inp).unwrap();
        assert_eq!(result.access_key_id, "ENV_AKID");
        assert_eq!(result.secret_access_key, "ENV_SAK");
        assert_eq!(source, CredentialSource::Environment);
    }

    #[test]
    fn profile_file_when_no_config_or_env() {
        let dir = tempfile::tempdir().unwrap();
        let cred_file = dir.path().join("credentials");
        {
            let mut f = std::fs::File::create(&cred_file).unwrap();
            writeln!(f, "[default]").unwrap();
            writeln!(f, "aws_access_key_id = PROFILE_AKID").unwrap();
            writeln!(f, "aws_secret_access_key = PROFILE_SAK").unwrap();
            writeln!(f, "region = us-west-2").unwrap();
        }

        let inp = input(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some("default"),
            Some(cred_file.as_path()),
        );
        let (result, source) = resolve_credentials(&inp).unwrap();
        assert_eq!(result.access_key_id, "PROFILE_AKID");
        assert_eq!(result.secret_access_key, "PROFILE_SAK");
        assert_eq!(result.region, "us-west-2");
        assert_eq!(source, CredentialSource::ProfileFile);
    }

    #[test]
    fn none_when_no_credentials_available() {
        let inp = input(None, None, None, None, None, None, None, None, None, None);
        assert!(resolve_credentials(&inp).is_none());
    }

    #[test]
    fn empty_config_values_fall_through() {
        let inp = input(
            Some(""),
            Some(""),
            None,
            None,
            Some("ENV_AKID"),
            Some("ENV_SAK"),
            None,
            None,
            None,
            None,
        );
        let (result, source) = resolve_credentials(&inp).unwrap();
        assert_eq!(result.access_key_id, "ENV_AKID");
        assert_eq!(source, CredentialSource::Environment);
    }

    #[test]
    fn default_region_when_not_specified() {
        let inp = input(
            Some("AKID"),
            Some("SAK"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let (result, _) = resolve_credentials(&inp).unwrap();
        assert_eq!(result.region, "us-east-1");
    }

    #[test]
    fn read_profile_with_session_token() {
        let dir = tempfile::tempdir().unwrap();
        let cred_file = dir.path().join("credentials");
        {
            let mut f = std::fs::File::create(&cred_file).unwrap();
            writeln!(f, "[my-profile]").unwrap();
            writeln!(f, "aws_access_key_id = AKID").unwrap();
            writeln!(f, "aws_secret_access_key = SAK").unwrap();
            writeln!(f, "aws_session_token = TOKEN").unwrap();
        }

        let creds = read_profile(&cred_file, "my-profile").unwrap();
        assert_eq!(creds.access_key_id, "AKID");
        assert_eq!(creds.session_token.as_deref(), Some("TOKEN"));
    }

    #[test]
    fn read_profile_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cred_file = dir.path().join("credentials");
        {
            let mut f = std::fs::File::create(&cred_file).unwrap();
            writeln!(f, "[other-profile]").unwrap();
            writeln!(f, "aws_access_key_id = AKID").unwrap();
        }

        let result = read_profile(&cred_file, "missing-profile");
        assert!(result.is_none());
    }

    #[test]
    fn read_profile_incomplete_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cred_file = dir.path().join("credentials");
        {
            let mut f = std::fs::File::create(&cred_file).unwrap();
            writeln!(f, "[incomplete]").unwrap();
            writeln!(f, "aws_access_key_id = AKID").unwrap();
            // No secret_access_key
        }

        let result = read_profile(&cred_file, "incomplete");
        assert!(result.is_none());
    }
}
