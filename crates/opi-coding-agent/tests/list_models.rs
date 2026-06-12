//! Behavioral tests for --list-models (task 2.1).
//!
//! Tests that `opi --list-models` exits with code 0 when at least one
//! provider has credentials, outputs model IDs, and supports --json.
//! Also tests graceful failure when no credentials are available.
//!
//! All tests run from a temp dir to avoid .env file loading.

use std::process::Command;

fn opi_bin() -> String {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent().unwrap().parent().unwrap();

    // Prefer debug (always fresh from `cargo test`), fall back to release.
    for profile in &["debug", "release"] {
        let mut path = workspace_root.join("target").join(profile).join("opi");
        if cfg!(windows) {
            path.set_extension("exe");
        }
        if path.exists() {
            return path.to_string_lossy().into_owned();
        }
    }

    // Fall back to debug path even if it doesn't exist yet (will fail clearly).
    let mut path = workspace_root.join("target/debug/opi");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    path.to_string_lossy().into_owned()
}

fn run_opi(args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let bin = opi_bin();
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(&bin);
    cmd.args(args).current_dir(tmp.path()).env_clear();
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"))
}

/// Run opi with a temp config file. The caller provides TOML content for the
/// config file; `envs` are set in addition to a clean environment.
fn run_opi_with_config(
    config_toml: &str,
    extra_args: &[&str],
    envs: &[(&str, &str)],
) -> std::process::Output {
    let bin = opi_bin();
    let tmp = tempfile::tempdir().unwrap();
    let config_path = tmp.path().join("test-config.toml");
    std::fs::write(&config_path, config_toml).unwrap();

    let mut args = vec![
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
    ];
    for a in extra_args {
        args.push((*a).to_string());
    }

    let mut cmd = Command::new(&bin);
    cmd.args(&args).current_dir(tmp.path()).env_clear();
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to run {bin}: {e}"))
}

#[test]
fn list_models_without_credentials_exits_nonzero() {
    let output = run_opi(&["--list-models"], &[]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit without credentials, got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("no models available"),
        "stderr should mention no models available, got: {stderr}"
    );
}

#[test]
fn list_models_with_anthropic_key_outputs_models() {
    let output = run_opi(
        &["--list-models"],
        &[("ANTHROPIC_API_KEY", "test-key-for-listing")],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0 with ANTHROPIC_API_KEY, got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("anthropic"),
        "output should mention 'anthropic' provider, got: {stdout}"
    );
    assert!(
        stdout.contains("claude"),
        "output should contain claude model IDs, got: {stdout}"
    );
}

#[test]
fn list_models_json_outputs_ndjson() {
    let output = run_opi(
        &["--list-models", "--json"],
        &[("ANTHROPIC_API_KEY", "test-key-for-listing")],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );

    let mut found_anthropic = false;
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("line is not valid JSON: {line}\nerror: {e}"));
        assert!(
            v.get("model").is_some(),
            "JSON line missing 'model' field: {line}"
        );
        assert!(
            v.get("provider").is_some(),
            "JSON line missing 'provider' field: {line}"
        );
        assert!(
            v.get("display_name").is_some(),
            "JSON line missing 'display_name' field: {line}"
        );
        if v["provider"].as_str() == Some("anthropic") {
            found_anthropic = true;
        }
    }
    assert!(
        found_anthropic,
        "expected at least one anthropic model in JSON output"
    );
}

#[test]
fn list_models_includes_provider_column() {
    let output = run_opi(
        &["--list-models"],
        &[("ANTHROPIC_API_KEY", "test-key-for-listing")],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}",
        output.status.code()
    );

    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 3,
        "expected at least header + separator + 1 model, got {} lines",
        lines.len()
    );
    assert!(
        lines[0].contains("PROVIDER"),
        "header should contain PROVIDER, got: {}",
        lines[0]
    );
    assert!(
        lines[0].contains("MODEL ID"),
        "header should contain MODEL ID, got: {}",
        lines[0]
    );
}

// ---------------------------------------------------------------------------
// Invalid proxy config must cause --list-models to exit with config error
// ---------------------------------------------------------------------------

#[test]
fn list_models_invalid_proxy_exits_config_error() {
    let output = run_opi_with_config(
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[providers.anthropic.proxy]
url = "not a proxy url"
"#,
        &["--list-models"],
        &[("ANTHROPIC_API_KEY", "test-key-for-listing")],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let code = output.status.code();

    assert!(
        code == Some(2),
        "expected exit code 2 for config error, got {code:?}\nstdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        stderr.contains("config error"),
        "stderr should mention config error, got: {stderr}",
    );
    assert!(
        stderr.contains("failed to build HTTP client with proxy config"),
        "stderr should mention proxy config failure, got: {stderr}",
    );
    assert!(
        stdout.is_empty(),
        "stdout should be empty on config error, got: {stdout}",
    );
}

#[test]
fn list_models_valid_proxy_with_credentials_succeeds() {
    // A well-formed proxy URL (nothing is listening, but config parsing succeeds).
    // This verifies that valid proxy config does not block --list-models.
    let output = run_opi_with_config(
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"

[providers.anthropic.proxy]
url = "http://proxy.example.com:8080"
"#,
        &["--list-models"],
        &[("ANTHROPIC_API_KEY", "test-key-for-listing")],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "expected exit 0 with valid proxy config, got {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("claude"),
        "output should contain claude model IDs, got: {stdout}",
    );
}

#[test]
fn list_models_missing_credentials_skips_provider_silently() {
    // No API key set for anthropic -- should skip silently, not error.
    // Another provider (openai) has a key, so we still get output.
    let output = run_opi_with_config(
        r#"
[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
"#,
        &["--list-models"],
        // No ANTHROPIC_API_KEY, but set OPENAI_API_KEY
        &[("OPENAI_API_KEY", "test-key-for-listing")],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should succeed since OpenAI has credentials
    assert!(
        output.status.success(),
        "expected exit 0, got {:?}",
        output.status.code()
    );
    // Should have openai models but not anthropic
    assert!(
        stdout.contains("openai"),
        "output should contain openai models, got: {stdout}",
    );
}

#[test]
fn list_models_openai_codex_behavior() {
    use base64::Engine;
    // 1. Without auth, openai-codex models should not appear
    let tmp = tempfile::tempdir().unwrap();

    let output = run_opi_with_config(
        r#"[defaults]
model = "openai:gpt-4o"
"#,
        &["--list-models"],
        &[
            ("OPI_AUTH_DIR", tmp.path().to_str().unwrap()),
            ("OPENAI_API_KEY", "test-key-for-listing"),
        ],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(!stdout.contains("openai-codex"));

    // 2. With auth, openai-codex models should appear
    let auth_dir = tmp.path().join("auth");
    std::fs::create_dir_all(&auth_dir).unwrap();

    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload_val = serde_json::json!({
        "chatgpt_account_id": "test-account-codex",
        "exp": 2999999999i64
    });
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_string(&payload_val).unwrap());
    let mock_jwt = format!("{}.{}.signature", header, payload);

    let auth_file_content = serde_json::json!({
        "openai-codex": {
            "type": "oauth",
            "oauth": {
                "accessToken": mock_jwt,
                "refreshToken": "ref-tok",
                "idToken": serde_json::Value::Null,
                "expiresAt": 2999999999i64,
                "accountId": "test-account-codex"
            },
            "issuer": "https://auth.openai.com",
            "clientId": "app_EMoamEEZ73f0CkXaXp7hrann"
        }
    });
    std::fs::write(
        auth_dir.join("auth.json"),
        serde_json::to_string_pretty(&auth_file_content).unwrap(),
    )
    .unwrap();

    let output_with_auth = run_opi_with_config(
        r#"[defaults]
model = "openai:gpt-4o"
"#,
        &["--list-models"],
        &[
            ("OPI_AUTH_DIR", auth_dir.to_str().unwrap()),
            ("OPENAI_API_KEY", "test-key-for-listing"),
        ],
    );

    let stdout_with_auth = String::from_utf8_lossy(&output_with_auth.stdout);
    assert!(output_with_auth.status.success());
    assert!(stdout_with_auth.contains("openai-codex"));
    assert!(stdout_with_auth.contains("gpt-5.5"));
}
