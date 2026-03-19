use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Once;
use tempfile::TempDir;

static DAEMON_INIT: Once = Once::new();
static mut DAEMON_PROCESS: Option<Child> = None;

fn worktree_sync_bin() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates
    path.pop(); // root
    path.push("target");
    path.push("debug");
    path.push("worktree-sync");
    path
}

fn ensure_daemon_running() {
    DAEMON_INIT.call_once(|| {
        // Kill any existing daemons first
        let _ = Command::new("pkill")
            .args(&["-9", "-f", "worktree-sync daemon"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(300));

        // Start singleton daemon
        let daemon = Command::new(worktree_sync_bin())
            .arg("daemon")
            .spawn()
            .expect("failed to start daemon");

        unsafe {
            DAEMON_PROCESS = Some(daemon);
        }

        // Give daemon time to start
        std::thread::sleep(std::time::Duration::from_millis(500));
    });
}

#[test]
fn cycle_color_fails_outside_git_worktree() {
    ensure_daemon_running();

    let dir = TempDir::new().unwrap();
    let non_git_path = dir.path();

    // Try to cycle color in non-git directory
    let output = Command::new(worktree_sync_bin())
        .args(&["cycle-color", "--worktree-path", non_git_path.to_str().unwrap()])
        .output()
        .expect("failed to run cycle-color");

    // Should fail
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "cycle-color should fail in non-git directory.\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    // Check error message (might be in stdout or stderr)
    let error_output = format!("{}{}", stdout, stderr);
    assert!(
        error_output.contains("Not a git worktree") || error_output.contains("git worktree"),
        "Error message should mention git worktree. Got: {}", error_output
    );
    assert!(
        error_output.contains("git-scm.com"),
        "Error message should include link to git documentation. Got: {}", error_output
    );
}

#[test]
fn cycle_color_works_in_regular_git_repo() {
    ensure_daemon_running();

    let dir = TempDir::new().unwrap();
    let repo_path = dir.path();

    // Initialize a regular git repo (not a worktree)
    Command::new("git")
        .args(&["init"])
        .current_dir(repo_path)
        .output()
        .expect("failed to init git repo");

    Command::new("git")
        .args(&["config", "user.name", "Test"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(&["config", "user.email", "test@test.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create initial commit
    std::fs::write(repo_path.join("test.txt"), "test").unwrap();
    Command::new("git")
        .args(&["add", "test.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(&["commit", "-m", "initial"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Run cycle-color - should succeed for regular repos
    let output = Command::new(worktree_sync_bin())
        .args(&["cycle-color", "--worktree-path", repo_path.to_str().unwrap()])
        .output()
        .expect("failed to run cycle-color");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "cycle-color should work in regular git repo.\nstdout: {}\nstderr: {}",
        stdout, stderr
    );

    // Verify settings file was created
    let settings_path = repo_path.join(".vscode/settings.json");
    assert!(settings_path.exists(), ".vscode/settings.json should be created");
}
