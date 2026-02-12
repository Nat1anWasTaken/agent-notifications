use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn temp_config_path(test_name: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time went backwards")
        .as_nanos();

    std::env::temp_dir()
        .join(format!("anot-tests-{pid}-{nanos}"))
        .join(test_name)
        .join("a-notifications.json")
}

fn run_anot_with_stdin(args: &[&str], stdin: &str, config_path: &PathBuf) -> Output {
    let exe = env!("CARGO_BIN_EXE_anot");

    let mut cmd = Command::new(exe);
    cmd.arg("--config")
        .arg(config_path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().expect("failed to spawn anot");
    {
        let mut child_stdin = child.stdin.take().expect("failed to open stdin");
        child_stdin
            .write_all(stdin.as_bytes())
            .expect("failed to write stdin");
    }

    child.wait_with_output().expect("failed to wait on anot")
}

#[test]
fn opencode_session_idle_missing_session_id_exits_nonzero() {
    let config_path = temp_config_path("missing-session-id");
    let output = run_anot_with_stdin(&["opencode"], r#"{"type":"session.idle"}"#, &config_path);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("sessionID is required for session.idle"));
}

#[test]
fn opencode_multiline_unknown_event_is_noop_success() {
    let config_path = temp_config_path("multiline-unknown");
    let output = run_anot_with_stdin(
        &["opencode"],
        r#"{
  "type": "something.else"
}
"#,
        &config_path,
    );

    assert!(output.status.success());
}

#[test]
fn opencode_invalid_json_exits_nonzero() {
    let config_path = temp_config_path("invalid-json");
    let output = run_anot_with_stdin(&["opencode"], "not-json", &config_path);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Invalid JSON"));
}

#[test]
fn opencode_session_error_succeeds_without_session_id() {
    let config_path = temp_config_path("session-error-no-session");
    let output = run_anot_with_stdin(
        &["opencode"],
        r#"{"type":"session.error","properties":{"error":{"name":"UnknownError","data":{"message":"boom"}}}}"#,
        &config_path,
    );

    assert!(output.status.success());
}
