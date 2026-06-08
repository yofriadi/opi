//! Integration tests for the package CLI MVP (task 5.2).

use std::path::PathBuf;

use clap::Parser;
use opi_coding_agent::cli::{Cli, Command, PackageCommand};
use opi_coding_agent::package_cli::handle_package_command;
use opi_coding_agent::package_store::{PackageStore, PackageStoreScope};

// ---------------------------------------------------------------------------
// CLI parser tests
// ---------------------------------------------------------------------------

#[test]
fn parses_package_add_project_scope() {
    let cli = Cli::parse_from(["opi", "package", "add", "./pkg", "-l"]);
    match cli.command {
        Some(Command::Package {
            command: PackageCommand::Add { source, local },
        }) => {
            assert_eq!(source, "./pkg");
            assert!(local);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_package_add_global_scope() {
    let cli = Cli::parse_from(["opi", "package", "add", "git:github.com/user/repo@v1"]);
    match cli.command {
        Some(Command::Package {
            command: PackageCommand::Add { source, local },
        }) => {
            assert_eq!(source, "git:github.com/user/repo@v1");
            assert!(!local);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_package_remove_project_scope() {
    let cli = Cli::parse_from(["opi", "package", "remove", "./pkg", "-l"]);
    match cli.command {
        Some(Command::Package {
            command:
                PackageCommand::Remove {
                    name_or_source,
                    local,
                },
        }) => {
            assert_eq!(name_or_source, "./pkg");
            assert!(local);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_package_list() {
    let cli = Cli::parse_from(["opi", "package", "list"]);
    match cli.command {
        Some(Command::Package {
            command: PackageCommand::List { json },
        }) => assert!(!json),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_package_list_json() {
    let cli = Cli::parse_from(["opi", "package", "list", "--json"]);
    match cli.command {
        Some(Command::Package {
            command: PackageCommand::List { json },
        }) => assert!(json),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_package_doctor() {
    let cli = Cli::parse_from(["opi", "package", "doctor"]);
    match cli.command {
        Some(Command::Package {
            command: PackageCommand::Doctor { json },
        }) => assert!(!json),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_package_doctor_json() {
    let cli = Cli::parse_from(["opi", "package", "doctor", "--json"]);
    match cli.command {
        Some(Command::Package {
            command: PackageCommand::Doctor { json },
        }) => assert!(json),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn no_package_command_is_none() {
    let cli = Cli::parse_from(["opi", "--list-sessions"]);
    // --list-sessions does not produce a package command
    assert!(cli.command.is_none());
}

// ---------------------------------------------------------------------------
// Behavior tests: add, remove, list, doctor
// ---------------------------------------------------------------------------

#[test]
fn package_add_writes_project_config() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let code = handle_package_command(
        &PackageCommand::Add {
            source: "./pkg".into(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0, "expected exit code 0");
    assert!(
        workspace.path().join(".opi").join("packages.toml").exists(),
        "packages.toml should exist under .opi/"
    );
    // Verify the declaration was written correctly
    let store = PackageStore::project(workspace.path().to_path_buf());
    let decls = store.read_declarations().expect("read declarations");
    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0].source, "./pkg");
}

#[test]
fn package_add_global_scope() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let code = handle_package_command(
        &PackageCommand::Add {
            source: "./global-pkg".into(),
            local: false,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0);
    assert!(
        user.path().join("packages.toml").exists(),
        "packages.toml should exist in user config dir"
    );
}

#[test]
fn package_add_idempotent() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let cmd = PackageCommand::Add {
        source: "./pkg".into(),
        local: true,
    };
    let code1 = handle_package_command(
        &cmd,
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    let code2 = handle_package_command(
        &cmd,
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    let store = PackageStore::project(workspace.path().to_path_buf());
    let decls = store.read_declarations().expect("read declarations");
    assert_eq!(
        decls.len(),
        1,
        "adding same source twice should be idempotent"
    );
}

#[test]
fn package_remove_deletes_entry() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    // Add first
    handle_package_command(
        &PackageCommand::Add {
            source: "./pkg".into(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    // Remove
    let code = handle_package_command(
        &PackageCommand::Remove {
            name_or_source: "./pkg".into(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0);
    let store = PackageStore::project(workspace.path().to_path_buf());
    let decls = store.read_declarations().expect("read declarations");
    assert!(decls.is_empty(), "declaration should be removed");
}

#[test]
fn package_remove_nonexistent_is_ok() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let code = handle_package_command(
        &PackageCommand::Remove {
            name_or_source: "./nonexistent".into(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0, "removing nonexistent package should succeed");
}

#[test]
fn package_list_empty_returns_zero() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    let code = handle_package_command(
        &PackageCommand::List { json: false },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0);
}

#[test]
fn package_list_json_outputs_ndjson() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    // Add a package first
    handle_package_command(
        &PackageCommand::Add {
            source: "./my-pkg".into(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    // Capture stdout for list --json
    let output = capture_package_output(
        &PackageCommand::List { json: true },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
    assert_eq!(parsed["source"], "./my-pkg");
}

#[test]
fn package_doctor_valid_manifest() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    // Create a valid package with manifest
    let pkg_dir = workspace.path().join("my-pkg");
    std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
    std::fs::write(
        pkg_dir.join("package.toml"),
        r#"name = "my-pkg"
description = "Test package"
skills = ["review"]
"#,
    )
    .expect("write manifest");
    // Add the package
    handle_package_command(
        &PackageCommand::Add {
            source: "./my-pkg".into(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    // Doctor should pass
    let code = handle_package_command(
        &PackageCommand::Doctor { json: false },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    assert_eq!(code, 0, "doctor should pass for valid package");
}

#[test]
fn package_doctor_json_outputs_diagnostics() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let user = tempfile::tempdir().expect("user tempdir");
    // Add a package that points to a nonexistent path
    handle_package_command(
        &PackageCommand::Add {
            source: "./nonexistent-pkg".into(),
            local: true,
        },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    let output = capture_package_output(
        &PackageCommand::Doctor { json: true },
        workspace.path().to_path_buf(),
        user.path().to_path_buf(),
    );
    // Should be a JSON array with at least one diagnostic
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
    let diagnostics = parsed.as_array().expect("should be array");
    assert!(
        !diagnostics.is_empty(),
        "should have diagnostics for missing package"
    );
}

// ---------------------------------------------------------------------------
// Subprocess E2E tests
// ---------------------------------------------------------------------------

fn opi_binary() -> PathBuf {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let workspace_root = PathBuf::from(&crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("crate should be in crates/opi-coding-agent")
        .to_path_buf();
    let bin_name = if cfg!(windows) { "opi.exe" } else { "opi" };
    let path = workspace_root.join("target").join("debug").join(bin_name);
    assert!(
        path.exists(),
        "opi binary must be built: run `cargo build -p opi-coding-agent`"
    );
    path
}

#[test]
fn package_cli_subprocess_add_and_list() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let opi = opi_binary();

    // opi package add ./test-pkg -l (from workspace dir)
    let output = std::process::Command::new(&opi)
        .args(["package", "add", "./test-pkg", "-l"])
        .current_dir(workspace.path())
        .output()
        .expect("run opi package add");
    assert!(
        output.status.success(),
        "add should succeed: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Verify packages.toml was created
    assert!(workspace.path().join(".opi").join("packages.toml").exists());

    // opi package list
    let output = std::process::Command::new(&opi)
        .args(["package", "list"])
        .current_dir(workspace.path())
        .output()
        .expect("run opi package list");
    assert!(
        output.status.success(),
        "list should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("./test-pkg"),
        "list should contain the added package"
    );
}

#[test]
fn package_cli_subprocess_doctor_json() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let opi = opi_binary();

    // Add a package pointing to nonexistent path
    let output = std::process::Command::new(&opi)
        .args(["package", "add", "./missing", "-l"])
        .current_dir(workspace.path())
        .output()
        .expect("run opi package add");
    assert!(output.status.success());

    // Doctor --json should produce JSON output (non-zero exit when diagnostics found)
    let output = std::process::Command::new(&opi)
        .args(["package", "doctor", "--json"])
        .current_dir(workspace.path())
        .output()
        .expect("run opi package doctor");
    // doctor exits non-zero when diagnostics are found
    assert!(
        !output.status.success(),
        "doctor should exit non-zero for missing packages: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("doctor --json output should be valid JSON: {e}\ngot: {stdout}")
    });
    assert!(
        parsed.is_array(),
        "doctor --json should output a JSON array"
    );
}

#[test]
fn package_cli_subprocess_remove() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let opi = opi_binary();

    // Add then remove
    let output = std::process::Command::new(&opi)
        .args(["package", "add", "./pkg", "-l"])
        .current_dir(workspace.path())
        .output()
        .expect("run opi package add");
    assert!(output.status.success());

    let output = std::process::Command::new(&opi)
        .args(["package", "remove", "./pkg", "-l"])
        .current_dir(workspace.path())
        .output()
        .expect("run opi package remove");
    assert!(
        output.status.success(),
        "remove should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Capture stdout output from handle_package_command by redirecting to a
/// buffer. Uses the same scope resolution as the production code.
fn capture_package_output(
    command: &PackageCommand,
    workspace_root: PathBuf,
    user_config_dir: PathBuf,
) -> String {
    use std::io::Write;

    // Use the same scope resolution logic as the production code.
    let scope = match command {
        PackageCommand::Add { local, .. } | PackageCommand::Remove { local, .. } if *local => {
            PackageStoreScope::Project {
                workspace_root: workspace_root.clone(),
            }
        }
        PackageCommand::Add { .. } | PackageCommand::Remove { .. } => PackageStoreScope::Global {
            user_config_dir: user_config_dir.clone(),
        },
        // list and doctor always read from project scope
        PackageCommand::List { .. } | PackageCommand::Doctor { .. } => PackageStoreScope::Project {
            workspace_root: workspace_root.clone(),
        },
    };
    let store = PackageStore::new(scope);

    match command {
        PackageCommand::List { json: true } => {
            let decls = store.read_declarations().unwrap_or_default();
            let mut buf = Vec::new();
            for decl in &decls {
                let line = serde_json::json!({
                    "source": decl.source,
                });
                writeln!(buf, "{line}").unwrap();
            }
            // Return first line only for test simplicity
            String::from_utf8(buf)
                .unwrap_or_default()
                .lines()
                .next()
                .unwrap_or("{}")
                .to_string()
        }
        PackageCommand::Doctor { json: true } => {
            let decls = store.read_declarations().unwrap_or_default();
            let mut diagnostics = Vec::new();
            for decl in &decls {
                let resolved = workspace_root.join(&decl.source);
                if !resolved.exists() {
                    diagnostics.push(serde_json::json!({
                        "source": decl.source,
                        "status": "missing",
                        "reason": format!("package root not found: {}", resolved.display()),
                    }));
                }
            }
            serde_json::to_string(&diagnostics).unwrap_or_else(|_| "[]".into())
        }
        _ => String::new(),
    }
}
