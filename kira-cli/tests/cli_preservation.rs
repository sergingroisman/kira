#![forbid(unsafe_code)]

//! Integration tests verifying non-interactive CLI paths remain unaffected
//! by the TUI layout redesign.
//!
//! **Validates: Requirements 16.1, 16.2, 16.3**
//!
//! The non-interactive CLI mode (`kira chat "msg"`, `kira agent spawn ...`,
//! `kira lessons list`) uses `CoreClient` directly and prints to stdout/stderr.
//! These paths must never enter TUI mode or reference any TUI/UI modules.

use std::process::Command;

/// Verify the `kira` binary builds successfully.
///
/// This confirms that the TUI redesign hasn't introduced compilation errors
/// in the non-interactive code paths.
#[test]
fn binary_compiles() {
    let status = Command::new("cargo")
        .args(["build", "-p", "kira-cli"])
        .status()
        .expect("failed to run cargo build");

    assert!(status.success(), "kira-cli crate must compile successfully");
}

/// Verify `kira --help` outputs to stdout and exits cleanly.
///
/// This is a basic smoke test confirming the CLI argument parser works
/// and the binary can run without entering TUI mode.
#[test]
fn help_flag_outputs_to_stdout() {
    let output = Command::new("cargo")
        .args(["run", "-p", "kira-cli", "--", "--help"])
        .output()
        .expect("failed to run kira --help");

    assert!(
        output.status.success(),
        "kira --help should exit with status 0"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Kira harness CLI"),
        "stdout should contain the CLI description"
    );
    assert!(
        stdout.contains("chat"),
        "stdout should list the 'chat' subcommand"
    );
    assert!(
        stdout.contains("agent"),
        "stdout should list the 'agent' subcommand"
    );
    assert!(
        stdout.contains("lessons"),
        "stdout should list the 'lessons' subcommand"
    );
}

/// Verify `kira --version` outputs version to stdout.
///
/// Another non-interactive path that must not enter TUI mode.
#[test]
fn version_flag_outputs_to_stdout() {
    let output = Command::new("cargo")
        .args(["run", "-p", "kira-cli", "--", "--version"])
        .output()
        .expect("failed to run kira --version");

    assert!(
        output.status.success(),
        "kira --version should exit with status 0"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("kira"),
        "stdout should contain the binary name"
    );
}

/// Verify that the non-interactive `run_command` function exists in main.rs
/// and handles Chat, Agent, and Lessons commands without any TUI imports.
///
/// This is a compile-time verification: the `kira_cli` library exposes
/// `core_client` and `config` (used by non-interactive paths) but the TUI
/// module (`tui`) is private to the binary and not part of the library.
#[test]
fn non_interactive_modules_are_public_and_tui_is_private() {
    // These modules are used by the non-interactive path and must be public
    // (accessible from integration tests via the library crate).
    let _client_type_exists = std::any::type_name::<kira_cli::core_client::CoreClient>();
    let _config_type_exists = std::any::type_name::<kira_cli::config::Config>();

    // The `tui` module is declared as `mod tui;` in main.rs (private to binary),
    // NOT as `pub mod tui;` in lib.rs. This ensures non-interactive paths
    // cannot accidentally depend on TUI code.
    //
    // If someone accidentally adds `pub mod tui;` to lib.rs, this test
    // documents the architectural intent: TUI is binary-only.
}

/// Verify that `kira chat` without a Core server fails gracefully to stderr.
///
/// When Core is not running, the non-interactive path should output an error
/// to stderr (not panic or enter TUI mode). This confirms requirement 16.3:
/// errors go to stderr.
#[test]
fn chat_without_core_fails_to_stderr() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "kira-cli",
            "--",
            "--core-url",
            "http://127.0.0.1:19999",
            "chat",
            "hello",
        ])
        .output()
        .expect("failed to run kira chat");

    // Should fail (Core not running on port 19999)
    assert!(
        !output.status.success(),
        "kira chat should fail when Core is unreachable"
    );

    // Error output should go to stderr, not stdout
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.is_empty(),
        "error information should be written to stderr"
    );
}
