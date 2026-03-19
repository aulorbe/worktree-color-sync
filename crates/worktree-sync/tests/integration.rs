use std::path::PathBuf;
use std::process::Command;

fn worktree_sync_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates
    path.pop(); // root
    path.push("target");
    path.push("debug");
    path.push("worktree-sync");
    path
}

#[test]
fn test_help_shows_all_commands() {
    let output = Command::new(worktree_sync_bin())
        .arg("--help")
        .output()
        .expect("failed to run --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("daemon"));
    assert!(stdout.contains("notify"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("current"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("cycle-color"));
}

#[test]
fn test_cycle_color_help() {
    let output = Command::new(worktree_sync_bin())
        .arg("cycle-color")
        .arg("--help")
        .output()
        .expect("failed to run cycle-color --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("worktree-path"));
    assert!(stdout.contains("defaults to current directory"));
}

#[test]
fn test_doctor_without_daemon() {
    let output = Command::new(worktree_sync_bin())
        .arg("doctor")
        .output()
        .expect("failed to run doctor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Doctor should work even without daemon running (local checks)
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("ok="));
}

#[test]
fn test_status_command_works() {
    let output = Command::new(worktree_sync_bin())
        .arg("status")
        .output()
        .expect("failed to run status");

    // If daemon is running, we should get status output
    // If not, we should get an error
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("running=") || stderr.contains("failed to connect"),
        "Expected either status output or connection error"
    );
}
